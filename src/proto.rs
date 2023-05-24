use std::io::{Read, Cursor};
use byteorder::{LittleEndian, ReadBytesExt};
use std::mem;
use anyhow::{bail, Context, Result};
use std::thread;
use std::sync::mpsc;
use std::collections::VecDeque;

pub struct MessageWatch {
    pending: VecDeque<u8>,
    rx: mpsc::Receiver<Vec<u8>>,
    next_msg: Option<MetaMessage>,
    next_data: Option<Vec<u8>>,
}

impl MessageWatch {
    pub fn new(input: impl Read + Send + 'static) -> Self {
        let (tx, rx) = mpsc::channel();
        thread::spawn(move || read_msgs(input, tx));
        Self {
            pending: VecDeque::new(),
            rx,
            next_msg: None,
            next_data: None,
        }
    }
    pub fn has_msg(&mut self) -> Result<bool> {
        self.update()?;

        Ok(self.next_msg.is_some() && self.next_data.is_some())
    }
    pub fn next(&mut self) -> Result<Message> {
        self.update()?;

        let Some(meta) = self.next_msg.take() else {
            bail!("No message cached");
        };

        let Some(data) = self.next_data.take() else {
            bail!("No data cached");
        };

        Ok(Message {
            meta,
            data,
        })
    }
    fn update(&mut self) -> Result<()> {
        for msg in self.rx.try_iter() {
            println!("{:x?}", msg);
            self.pending.append(&mut msg.into());
        }

        if self.next_msg.is_none() && 
                self.pending.len() >= mem::size_of::<MetaMessage>() {
            let msg: MetaMessage;
            let vec = self.pending.drain(0..24)
                .collect::<Vec<u8>>();
            assert_eq!(vec.len(), mem::size_of::<MetaMessage>());
            let mut cursor = Cursor::new(vec);
            
            let cmd = CommandType::try_from((
                    cursor.read_u32::<LittleEndian>()?,
                    cursor.read_u32::<LittleEndian>()?,
                    cursor.read_u32::<LittleEndian>()?))
                .context("Failed to get command type")?;
            msg = MetaMessage {
                cmd,
                len: cursor.read_u32::<LittleEndian>()?,
                crc: cursor.read_u32::<LittleEndian>()?,
                magic: cursor.read_u32::<LittleEndian>()?,
            };
            self.next_msg = Some(msg);
        }

        if let Some(msg) = &self.next_msg {
            if self.pending.len() >= msg.len as usize {
                let vec = self.pending.drain(..msg.len as usize)
                    .collect::<Vec<u8>>();
                self.next_data = Some(vec);
            } 
        }
        Ok(())
    }
}

fn read_msgs(mut from: impl Read, tx: mpsc::Sender<Vec<u8>>) {
    loop {
        let mut buf = [0; MAXDATA as usize];
        let n = from.read(&mut buf)
            .expect("Failed to read message");
        if n == 0 {
            std::thread::sleep(std::time::Duration::from_secs(1));
        }
        let mut vec = buf.to_vec();
        vec.resize(n, 0);
        if tx.send(vec).is_err() {
            break;
        }
    }
}

#[derive(Debug, Clone)]
#[repr(C)]
pub struct MetaMessage {
    cmd: CommandType,
    len: u32,
    crc: u32,
    magic: u32,
}

static_assertions::assert_eq_size!(MetaMessage, [u8; 24]);
static_assertions::assert_eq_size!(CommandType, [u32; 3]);

impl MetaMessage {
    pub fn bytes(&self) -> &[u8; 24] {
        unsafe {mem::transmute(self)}
    }
}

#[derive(Debug, Clone)]
#[repr(C)]
pub struct Message {
    meta: MetaMessage,
    data: Vec<u8>,
}

pub const MAXDATA: u32 = 256 * 1024;
pub const ADB_VERSION: u32 = 0x01000001;

impl Message {
    pub fn meta(&self) -> &MetaMessage { &self.meta }
    pub fn data(&self) -> &[u8] { &self.data }
    pub fn connect(version: u32, maxdata: u32, sysident: &[u8]) -> Self {
        let data = sysident.to_vec();
        let cmd = CommandType::Connect{version, maxdata};
        let magic = cmd.magic();
        Self {
            meta: MetaMessage {
                cmd,
                crc: 0,
                len: data.len() as u32,
                magic,
            },
            data,
        }
    }
    pub fn to_bytes(self) -> (Vec<u8>, Vec<u8>) {
        (self.meta.bytes().to_vec(), self.data)
    }
}

pub const A_CNXN: u32 = 0x4e584e43;
pub const A_AUTH: u32 = 0x48545541;
pub const A_OPEN: u32 = 0x4e45504f;
pub const A_OKAY: u32 = 0x59414b4f;
pub const A_CLSE: u32 = 0x45534c43;
pub const A_WRTE: u32 = 0x45545257;
pub const A_STLS: u32 = 0x534C5453;

pub const A_CNXN_MAGIC: u32 = 0xffffffff ^ A_CNXN;
pub const A_AUTH_MAGIC: u32 = 0xffffffff ^ A_AUTH;
pub const A_OPEN_MAGIC: u32 = 0xffffffff ^ A_OPEN;
pub const A_OKAY_MAGIC: u32 = 0xffffffff ^ A_OKAY;
pub const A_CLSE_MAGIC: u32 = 0xffffffff ^ A_CLSE;
pub const A_WRTE_MAGIC: u32 = 0xffffffff ^ A_WRTE;
pub const A_STLS_MAGIC: u32 = 0xffffffff ^ A_STLS;

#[derive(Debug, Clone)]
#[repr(u32, C)]
pub enum CommandType {
    Connect{version: u32, maxdata: u32} = A_CNXN,
    Stls{ty: u32, version: u32} = A_STLS,
    Auth{ty: u32, zero: u32} = A_AUTH,
    Open{local_id: u32, zero: u32} = A_OPEN,
    Ready{local_id: u32, remote_id: u32} = A_OKAY,
    Write{local_id: u32, remote_id: u32} = A_WRTE,
    Close{local_id: u32, remote_id: u32} = A_CLSE,
}

impl CommandType {
    fn magic(&self) -> u32 {
        use CommandType::*;
        match self {
            Connect{..} => A_CNXN_MAGIC,
            Stls{..} => A_STLS_MAGIC,
            Auth{..} => A_AUTH_MAGIC,
            Open{..} => A_OPEN_MAGIC,
            Ready{..} => A_OKAY_MAGIC,
            Write{..} => A_WRTE_MAGIC,
            Close{..} => A_CLSE_MAGIC,
        }
    }
    fn name(&self) -> &'static str{
        use CommandType::*;
        match self {
            Connect{..} => "Connect",
            Stls{..} => "Stls",
            Auth{..} => "Auth",
            Open{..} => "Open",
            Ready{..} => "Ready",
            Write{..} => "Write",
            Close{..} => "Close",
        }
    }
}

impl TryFrom<(u32, u32, u32)> for CommandType {
    type Error = anyhow::Error;

    fn try_from(vals: (u32, u32, u32)) -> Result<Self, Self::Error> {
        use CommandType::*;
        let cmd = vals.0;
        let arg1 = vals.1;
        let arg2 = vals.2;

        Ok(match cmd {
            A_CNXN => Connect{version: arg1, maxdata: arg2},
            A_STLS => Stls{ty: arg1, version: arg2},
            A_AUTH => Auth{ty: arg1, zero: arg2},
            A_OPEN => Open{local_id: arg1, zero: arg2},
            A_OKAY => Ready{local_id: arg1, remote_id: arg2},
            A_WRTE => Write{local_id: arg1, remote_id: arg2},
            A_CLSE => Close{local_id: arg1, remote_id: arg2},
            other  => bail!("Invalid cmd id {:x}", other),
        })
    }
}

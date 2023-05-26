use std::io::{self, Read, Cursor};
use byteorder::{LittleEndian, ReadBytesExt};
use std::mem;
use anyhow::{bail, Context, Result};

pub fn next_msg(from: &mut impl Read) -> Result<Message> {
    let meta;
    let mut read = 0;
    let mut buf = [0; MAXDATA as usize];

    while read < mem::size_of::<MetaMessage>() {
        read += from.read(&mut buf).expect("Failed to read message");
    }
    let mut vec = buf.to_vec();
    vec.resize(mem::size_of::<MetaMessage>(), 0);
    let mut cursor = Cursor::new(vec);

    let cmd = CommandType::try_from((
            cursor.read_u32::<LittleEndian>()?,
            cursor.read_u32::<LittleEndian>()?,
            cursor.read_u32::<LittleEndian>()?))
        .context("Failed to get command type")?;
    meta = MetaMessage {
        cmd,
        len: cursor.read_u32::<LittleEndian>()?,
        crc: cursor.read_u32::<LittleEndian>()?,
        magic: cursor.read_u32::<LittleEndian>()?,
    };

    let mut data;

    if meta.len > 0 {
        let mut buf = [0; MAXDATA as usize];
        read = cursor.read(&mut buf).unwrap();
        while read < meta.len as usize {
            read += from.read(&mut buf).expect("Failed to read data");
        }
        assert_eq!(meta.len as usize, read);

        data = buf.to_vec();
        data.resize(meta.len as usize, 0);
    } else {
        data = Vec::new();
    }

    Ok(Message {
        meta,
        data,
    })
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
    pub fn cmd(&self) -> &CommandType { &self.cmd }
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
    pub fn ready(local_id: u32, remote_id: u32) -> Self {
        let cmd = CommandType::Ready{local_id, remote_id};
        Self::mk_msg(cmd, Vec::new())
    }
    pub fn write(local_id: u32, remote_id: u32, data: Vec<u8>) -> Self {
        let cmd = CommandType::Write{local_id, remote_id};
        Self::mk_msg(cmd, data)
    }
    pub fn close(local_id: u32, remote_id: u32) -> Self {
        let cmd = CommandType::Close{local_id, remote_id};
        Self::mk_msg(cmd, Vec::new())
    }
    fn mk_msg(cmd: CommandType, data: Vec<u8>) -> Self {
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
    pub fn send_to(self, to_where: &mut impl io::Write) -> Result<()> {
        println!("tx: {:#x?}", self.meta());
        let (header, data) = self.to_bytes();
        to_where.write_all(&header)
            .context("Failed to write header")?;
        to_where.write_all(&data)
            .context("Failed to write payload")?;
        Ok(())
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
    /*fn name(&self) -> &'static str{
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
    }*/
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

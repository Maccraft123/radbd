use std::io::Write;
use std::sync::Weak;
use std::env;
use crossbeam_channel::{Sender, Receiver};
use std::collections::VecDeque;

use anyhow::Result;
use crate::proto::{Message, CommandType};

pub mod shell;
pub mod sync;

pub struct Service {
    tx: Sender<Vec<u8>>,
    rx: Receiver<Vec<u8>>,
    ptr: Weak<()>,
}

pub struct Stream {
    id: u32,
    remote_id: u32,
    svc: Service,
    pending_msgs: VecDeque<Message>,
    sent_ready: bool,
    ok_to_write: bool,
}

impl Stream {
    pub fn new(id: u32, remote_id: u32, svc: Service) -> Self {
        Self {
            id,
            remote_id,
            svc,
            pending_msgs: VecDeque::new(),
            sent_ready: false,
            ok_to_write: true,
        }
    }
    pub fn tick(&mut self, mut out: &mut impl Write) -> Result<bool> {
        if !self.sent_ready {
            Message::ready(self.id, self.remote_id).send_to(&mut out)?;
            self.sent_ready = true;
        }

        for vec in self.svc.rx.try_iter() {
            let msg = Message::write(self.id, self.remote_id, vec);
            self.pending_msgs.push_back(msg);
        }

        if self.ok_to_write {
            if let Some(msg) = self.pending_msgs.pop_front() {
                msg.send_to(out)?;
                self.ok_to_write = false;
            }
        }

        if self.svc.ptr.strong_count() == 0 {
            if self.pending_msgs.is_empty()  {
                println!("Closing stream {}", self.id);
                Message::close(self.id, self.remote_id).send_to(&mut out)?;
                return Ok(true);
            }
        }

        Ok(false)
    }
    pub fn handle_msg(&mut self, msg: Message) -> Result<()> {
        match msg.meta().cmd() {
            CommandType::Ready{remote_id, ..} => {
                if self.id == *remote_id {
                    self.ok_to_write = true;
                }
            },
            CommandType::Write{..} => {
                let data = msg.data().to_vec();
                self.svc.tx.send(data)?;
            }
            _ => (),
        }
        Ok(())
    }
    #[allow(dead_code)]
    pub fn id(&self) -> u32 { self.id }
    #[allow(dead_code)]
    pub fn remote_id(&self) -> u32 { self.remote_id }
}

pub fn spawn(id: u32, remote_id: u32, which: String) -> Result<Stream> {
    let which = which.trim_matches('\0');
    let split = which.split(':');
    let vec: Vec<&str> = split.collect();
    let name = vec.get(0).unwrap();
    let arg = vec.get(1).unwrap();

    let ret = match *name {
        "shell" => if arg == &"" {
            shell::start(env::var("SHELL").unwrap_or("sh".to_string()))?
        } else {
            shell::start(arg.to_string())?
        },
        "sync" => sync::start()?,
        _ => todo!("{:?}", which),
    };

    Ok(Stream::new(id, remote_id, ret))
}

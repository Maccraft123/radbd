use std::io::Write;
use std::env;
use crossbeam_channel::Receiver;
use std::collections::VecDeque;

use anyhow::Result;
use crate::proto::{Message, CommandType};

pub mod shell;
pub mod sync;
use shell::ShellService;
use sync::SyncService;

pub trait Service {
    fn handle_write(&mut self, data: Vec<u8>) -> Result<()>;
    fn recv(&mut self) -> &mut Receiver<Vec<u8>>;
    fn is_done(&mut self) -> bool;
    fn close(&mut self) -> Result<()> { Ok(()) }
}

pub struct Stream {
    id: u32,
    remote_id: u32,
    svc: Box<dyn Service>,
    pending_msgs: VecDeque<Message>,
    sent_ready: bool,
    ok_to_write: bool,
    die: bool,
}

impl Stream {
    pub fn new(id: u32, remote_id: u32, svc: Box<dyn Service>) -> Self {
        Self {
            id,
            remote_id,
            svc,
            pending_msgs: VecDeque::new(),
            sent_ready: false,
            ok_to_write: true,
            die: false,
        }
    }
    pub fn tick(&mut self, mut out: &mut impl Write) -> Result<bool> {
        if !self.sent_ready {
            Message::ready(self.id, self.remote_id).send_to(&mut out)?;
            self.sent_ready = true;
        }

        for vec in self.svc.recv().try_iter() {
            let msg = Message::write(self.id, self.remote_id, vec);
            self.pending_msgs.push_back(msg);
        }

        if self.ok_to_write {
            if let Some(msg) = self.pending_msgs.pop_front() {
                msg.send_to(out)?;
                self.ok_to_write = false;
            }
        }

        if self.svc.is_done() || self.die {
            if self.pending_msgs.is_empty()  {
                println!("Closing stream {}", self.id);
                self.svc.close()?;
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
                self.svc.handle_write(data)?;
                self.sent_ready = false;
            }
            _ => (),
        }
        Ok(())
    }
    pub fn schedule_death(&mut self) {
        self.die = true;
    }
}

pub fn spawn(id: u32, remote_id: u32, which: String) -> Result<Stream> {
    let which = which.trim_matches('\0');
    let split = which.split(':');
    let vec: Vec<&str> = split.collect();
    let name = vec.get(0).unwrap();
    let arg = vec.get(1).unwrap();

    let ret = match *name {
        "shell" => if arg == &"" {
            ShellService::start(env::var("SHELL").unwrap_or("sh".to_string()))?
        } else {
            ShellService::start(arg.to_string())?
        },
        "sync" => SyncService::start()?,
        _ => todo!("{:?}", which),
    };

    Ok(Stream::new(id, remote_id, ret))
}

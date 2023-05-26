use std::io::Write;
use crossbeam_channel::{TryRecvError, Sender, Receiver};
use std::collections::VecDeque;

use anyhow::Result;
use crate::proto::{Message, CommandType};

pub mod shell;

pub struct Stream {
    id: u32,
    remote_id: u32,
    tx: Sender<Vec<u8>>,
    rx: Receiver<Vec<u8>>,
    pending_msgs: VecDeque<Message>,
    sent_ready: bool,
    ok_to_write: bool,
}

impl Stream {
    pub fn new(id: u32, remote_id: u32, (tx, rx): (Sender<Vec<u8>>, Receiver<Vec<u8>>)) -> Self {
        Self {
            id,
            remote_id,
            tx,
            rx,
            pending_msgs: VecDeque::new(),
            sent_ready: false,
            ok_to_write: true,
        }
    }
    pub fn tick(&mut self, mut out: &mut impl Write) -> Result<()> {
        if !self.sent_ready {
            Message::ready(self.id, self.remote_id).send_to(&mut out)?;
            self.sent_ready = true;
        }

        for vec in self.rx.try_iter() {
            let msg = Message::write(self.id, self.remote_id, vec);
            self.pending_msgs.push_back(msg);
        }

        if self.ok_to_write {
            if let Some(msg) = self.pending_msgs.pop_front() {
                msg.send_to(out)?;

                if let Err(e) = self.rx.try_recv() {
                    if self.pending_msgs.is_empty() && e == TryRecvError::Disconnected {
                        Message::close(self.id, self.remote_id).send_to(&mut out)?;
                    }
                }
            }
        }

        Ok(())
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
                self.tx.send(data)?;
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

    let (tx, rx) = match *name {
        "shell" => if arg == &"" {
            shell::single_shot("bash".to_string())?
        } else {
            shell::single_shot(arg.to_string())?
        },

        other => unimplemented!("{other}"),
    };

    Ok(Stream::new(id, remote_id, (tx, rx)))
}

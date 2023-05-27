use crate::svc::Service;
use std::sync::Arc;
use std::thread;
use std::path::PathBuf;
use std::mem;
use nix::sys::stat::{stat, mode_t, Mode};
use crossbeam_channel::Receiver;
use anyhow::{bail, Result};

#[derive(Debug, Clone)]
#[repr(u32)]
enum Request {
    List,
    Recv,
    Send{mode: Mode, data: Vec<u8>, append: bool},
    Stat,
    Quit,
}

#[derive(Debug, Clone)]
enum Response {
    Stat{mode: u32, size: u32, mtime: u32},
    Fail,
    Okay,
}

impl Response {
    fn to_bytes(self) -> Vec<u8> {
        let mut ret = Vec::new();
        match self {
            Response::Stat{mode, size, mtime} => {
                ret.extend(b"STAT");
                ret.extend(u32::to_le_bytes(mode));
                ret.extend(u32::to_le_bytes(size));
                ret.extend(u32::to_le_bytes(mtime));
            },
            Response::Fail => ret.extend(b"FAIL"),
            Response::Okay => ret.extend(b"OKAY"),
        };
        ret
    }
}

enum State {
    Normal,
    SendMoreData,
}

struct SyncService {
    state: State,
}

impl SyncService {
    fn new() -> Self {
        Self { state: State::Normal }
    }
    fn get_next_cmd(&mut self, from: &Receiver<Vec<u8>>) -> Result<(Request, PathBuf)> {
        let packet = from.recv()?;
        if packet.len() < 8 {
            bail!("Invalid sync packet length");
        }

        match &packet[0..4] {
            //b"LIST" => Request::List,
            //b"RECV" => Request::Recv,
            b"SEND" => {
                let len = u32::from_le_bytes([packet[4], packet[5], packet[6], packet[7]]) as usize;
                let path_mode_str = String::from_utf8_lossy(&packet[8..][..len]).to_string();
                let path_mode: Vec<&str> = path_mode_str.split(',').collect();
                let path = path_mode.get(0).unwrap();
                let mode_raw = path_mode.get(1).unwrap().parse::<u32>()? as mode_t;
                let mode = Mode::from_bits_truncate(mode_raw);

                if mode.bits() != mode_raw {
                    eprintln!("Unsupported bits found: {:x}", mode.bits() ^ mode_raw);
                }

                let mut data = Vec::new();
                let mut offset = 8 + len;

                println!("{:x?}", String::from_utf8_lossy(&packet[offset..]));

                if packet[offset..][..4] != *b"DATA" {
                    bail!("Invalid stuff found idk i'm tired you write a better error message");
                }
                offset += 4;

                let data_len = u32::from_le_bytes([packet[offset],
                                                  packet[offset + 1],
                                                  packet[offset + 2],
                                                  packet[offset + 3]]) as usize;
                offset += 4;

                data.extend(&packet[offset..][..data_len]);
                offset += data_len;
                let append;

                if packet.len() != offset {
                    if packet[offset..][..4] != *b"DONE" {
                        bail!("Expected {:x?}(DONE), found {:x?}", b"DONE", &packet[offset..][..4])
                    }
                    offset += 4;
                    // Note: now at [offset..][..4] there is a u32 for creation time. we don't care.

                    self.state = State::Normal;
                    append = false;
                } else {
                    self.state = State::SendMoreData;
                    append = true;
                }

                println!("{:?} {:x?}", path, mode);

                Ok((Request::Send{mode, data, append}, PathBuf::from(path)))
            },
            b"STAT" => {
                let len = u32::from_le_bytes([packet[4], packet[5], packet[6], packet[7]]);
                let path = String::from_utf8_lossy(&packet[8..]).to_string();

                if path.len() != len as usize {
                    bail!("Length of path({:x}) isn't equal to length expected({:x})", path.len(), len)
                }
                Ok((Request::Stat, PathBuf::from(path)))
            },
            b"QUIT" => {
                Ok((Request::Quit, PathBuf::from("/dev/null")))
            }
            unknown => {
                bail!("Unknown sync cmd {:x?}", String::from_utf8_lossy(unknown));
            },
        }
    }
}

pub fn start() -> Result<Service> {
    let (ret_tx, input) = crossbeam_channel::unbounded::<Vec<u8>>();
    let (output, ret_rx) = crossbeam_channel::unbounded::<Vec<u8>>();

    let arc = Arc::new(());
    let ptr = Arc::downgrade(&arc);

    thread::spawn(move || {
        let mut svc = SyncService::new();
        loop {
            let (cmd, path) = svc.get_next_cmd(&input).unwrap();
            println!("{:?}", cmd);

            let response = match cmd {
                Request::Stat => {
                    let stat = stat(&path).unwrap();
                    Response::Stat{
                        size: stat.st_size as u32,
                        mode: stat.st_mode,
                        mtime: stat.st_mtime as u32,
                    }
                },
                Request::Send{..} => {
                    eprintln!("Writing into {:?} goes here", path);

                    Response::Okay
                }
                Request::Quit => {
                    break;
                }
                _ => todo!(),
            };

            output.send(response.to_bytes());
        }
        
        let ptr = Arc::downgrade(&arc);
        println!("Quitting sync, strong count {}", ptr.strong_count());
        drop(arc);
        println!("Now {}", ptr.strong_count());
    });

    Ok(Service {
        tx: ret_tx,
        rx: ret_rx,
        ptr,
    })
}

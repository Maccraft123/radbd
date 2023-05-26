mod proto;
mod usb;
mod svc;

use std::{
    fs::OpenOptions,
    io::Write,
    path::PathBuf,
    env,
    collections::HashMap,
    thread,
    time::Duration,
    sync::atomic::{AtomicBool, Ordering},
};
use anyhow::{Context, Result};
use proto::{CommandType, Message};
use svc::Stream;
use crossbeam_channel::select;

fn main() -> Result<()> {
    let endpoint_path = PathBuf::from(env::args().skip(1).next()
        .expect("First argument has to be functionfs path"));

    let mut ep_control = OpenOptions::new()
        .read(true)
        .write(true)
        .create(false)
        .open(endpoint_path.join("ep0"))
        .context("Failed to open ep0")?;

    ep_control.write_all(usb::ADB_DESCRIPTOR_V2.as_bytes())?;
    ep_control.write_all(usb::ADB_STRINGS.as_bytes())?;

    let mut ep_out = OpenOptions::new()
        .read(true)
        .write(false)
        .create(false)
        .open(endpoint_path.join("ep1"))
        .context("Failed to open ep1")?;

    let mut ep_in = OpenOptions::new()
        .read(false)
        .write(true)
        .create(false)
        .open(endpoint_path.join("ep2"))
        .context("Failed to open ep2")?;

    let connected = AtomicBool::new(false);
    thread::scope(|s| {
        s.spawn(|| {
            while !connected.load(Ordering::Acquire) {
                Message::connect(proto::ADB_VERSION, proto::MAXDATA, b"device:RIIR:Rewrite it in Rust\0").send_to(&mut ep_in)
                    .expect("Failed to send connect message");
                thread::sleep(Duration::from_secs(1));
            }
        });

        loop {
            let d = proto::next_msg(&mut ep_out).expect("Failed to read next message");
            match d.meta().cmd() {
                CommandType::Connect{..} => break,
                _ => continue,
            }
        }

        connected.store(true, Ordering::Release);
    });

    println!("Connected!");
    let mut streams: HashMap<u32, Stream> = HashMap::new();
    let mut next_id = 3;
    let (tx, rx) = crossbeam_channel::unbounded();

    thread::scope(|s| {
        s.spawn(|| {
            loop {
                let msg = proto::next_msg(&mut ep_out)
                    .expect("Failed to read next msg");
                if tx.send(msg).is_err() {
                    break;
                };
            }
        });

        loop {
            select!(
                recv(rx) -> msg => {
                    let msg = msg.unwrap();
                    println!("rx: {:#x?}", msg.meta());
                    match msg.meta().cmd() {
                        CommandType::Open{local_id, ..} => {
                            let name = String::from_utf8_lossy(msg.data());
                            let stream = svc::spawn(next_id, *local_id, name.to_string())
                                .expect("Failed to spawn a service");
                            streams.insert(next_id, stream);
                            next_id += 1;
                        }
                        CommandType::Ready{remote_id, ..} | CommandType::Write{remote_id, ..} => {
                            let stream = streams.get_mut(&remote_id).unwrap();
                            stream.handle_msg(msg)
                                .expect("Failed to handle a message");
                        }
                        CommandType::Close{remote_id, ..} => {
                            streams.remove(&remote_id).unwrap();
                        }
                        other => {
                            todo!("{:?}", other);
                        }
                    }

                    for (_, stream) in streams.iter_mut() {
                        stream.tick(&mut ep_in)
                            .expect("Failed to tick a stream");
                    }
                },
                default(Duration::from_millis(100)) => {
                    for (_, stream) in streams.iter_mut() {
                        stream.tick(&mut ep_in)
                            .expect("Failed to tick a stream");
                    }
                },
            );
        }
    });

    Ok(())
}

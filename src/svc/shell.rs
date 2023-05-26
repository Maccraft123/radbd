use std::time::Duration;
use std::thread;
use std::io::{Read, Write};
use crate::proto::MAXDATA;
use crossbeam_channel::{Sender, Receiver};
use portable_pty::{native_pty_system, PtySize, CommandBuilder};
use anyhow::Result;

fn cp_stream_to_chan(mut from: impl Read, to: Sender<Vec<u8>>) {
    loop {
        let mut buf = vec![0; MAXDATA as usize];
        let Ok(n) = from.read(&mut buf) else { return; };
        if n == 0 { return; }
        buf.resize(n, 0);

        println!("shell out: {:?}", String::from_utf8_lossy(&buf));
    
        if to.send(buf).is_err() {
            break;
        }
    }
}

fn cp_chan_to_stream(from: Receiver<Vec<u8>>, mut to: impl Write) {
    for vec in from.iter() {
        println!("shell in: {:?}", String::from_utf8_lossy(&vec));
        if to.write_all(&vec).is_err() {
            break;
        }
    }
}

pub fn start(cmd_args: String) -> Result<(Sender<Vec<u8>>, Receiver<Vec<u8>>)> {
    let (ret_tx, input) = crossbeam_channel::unbounded();
    let (output, ret_rx) = crossbeam_channel::unbounded();

    thread::spawn(move || {
        let pair = native_pty_system().openpty(PtySize {
            rows: 24,
            cols: 80,
            pixel_width: 0,
            pixel_height: 0
        }).unwrap();
        let cmd = CommandBuilder::new(cmd_args);
        let mut child = pair.slave.spawn_command(cmd).unwrap();

        let reader = pair.master.try_clone_reader().unwrap();
        let writer = pair.master.take_writer().unwrap();

        thread::spawn(move || cp_chan_to_stream(input, writer));
        thread::spawn(move || cp_stream_to_chan(reader, output));

        loop {
            let status = child.try_wait().unwrap();
            if status.is_some() {
                child.wait().unwrap();
                break;
            } else {
                thread::sleep(Duration::from_millis(100));
            }
        }
    });

    Ok((ret_tx, ret_rx))
}

use std::process::{Command, Stdio};
use std::time::Duration;
use std::thread;
use std::sync::mpsc::{self, Sender, Receiver};
use std::io::{Read, Write};
use crate::proto::MAXDATA;
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

pub fn single_shot(cmd_args: String) -> Result<(Sender<Vec<u8>>, Receiver<Vec<u8>>)> {
    let (ret_tx, input) = mpsc::channel();
    let (output, ret_rx) = mpsc::channel();

    thread::spawn(move || {
        let mut child = Command::new("bash")
            .arg("-c")
            .arg(cmd_args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("Failed to spawn bash");
        let output_2 = output.clone();
    
        let stdin = child.stdin.take().unwrap();
        let stdout = child.stdout.take().unwrap();
        let stderr = child.stderr.take().unwrap();
        thread::spawn(move || cp_chan_to_stream(input, stdin));
        thread::spawn(move || cp_stream_to_chan(stdout, output));
        thread::spawn(move || cp_stream_to_chan(stderr, output_2));

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

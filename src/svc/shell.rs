use std::thread;
use std::io::{Read, Write};
use crate::proto::MAXDATA;
use crate::svc::Service;
use crossbeam_channel::{Sender, Receiver};
use portable_pty::{Child, native_pty_system, PtySize, CommandBuilder};
use anyhow::Result;

pub struct ShellService {
    rx: Receiver<Vec<u8>>,
    child_stdin: Box<dyn Write>,
    child: Box<dyn Child>,
}

impl Service for ShellService {
    fn handle_write(&mut self, data: Vec<u8>) -> Result<()> {
        self.child_stdin.write_all(&data)?;
        Ok(())
    }
    #[allow(unused_must_use)]
    fn close(&mut self) -> Result<()> {
        self.child.kill();
        self.child.wait()?;
        Ok(())
    }
    fn recv(&mut self) -> &mut Receiver<Vec<u8>> {
        &mut self.rx
    }
    fn is_done(&mut self) -> bool {
        if let Ok(result) = self.child.try_wait() {
            result.is_some()
        } else {
            true
        }
    }
}

impl ShellService {
    pub fn start(cmd_args: String) -> Result<Box<dyn Service>> {
        let (tx, rx) = crossbeam_channel::unbounded();

        let pair = native_pty_system().openpty(PtySize {
            rows: 24,
            cols: 80,
            pixel_width: 0,
            pixel_height: 0
        })?;

        let mut cmd = CommandBuilder::new("bash");
        cmd.arg("-c");
        cmd.arg(cmd_args);

        let child = pair.slave.spawn_command(cmd)?;
        let child_stdin = pair.master.take_writer().unwrap();
        let child_stdout = pair.master.try_clone_reader().unwrap();

        thread::spawn(|| cp_stream_to_chan(child_stdout, tx));

        Ok(Box::new(Self {
            rx,
            child_stdin,
            child,
        }))
    }
}

fn cp_stream_to_chan(mut from: impl Read, to: Sender<Vec<u8>>) {
    loop {
        let mut buf = vec![0; MAXDATA as usize];
        let Ok(n) = from.read(&mut buf) else { return; };
        if n == 0 { return; }
        buf.resize(n, 0);
    
        if to.send(buf).is_err() {
            break;
        }
    }
}

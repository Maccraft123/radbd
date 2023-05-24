mod proto;
mod usb;

use std::{
    fs::OpenOptions,
    io::Write,
    path::PathBuf,
    env,
};
use anyhow::{Context, Result};
use proto::Message;

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

    let ep_out = OpenOptions::new()
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

    let mut w = proto::MessageWatch::new(ep_out);

    loop {
        if w.has_msg()? {
            let d = w.next()
                .context("Failed to read next message")?;
            println!("{:#x?}", d.meta());
            println!("{}", String::from_utf8_lossy(d.data()));
        } else {

            let msg = Message::connect(proto::ADB_VERSION, proto::MAXDATA, b"device:RIIR:Rewrite it in Rust\0");
            let (header, data) = msg.to_bytes();
            ep_in.write_all(&header)
                .context("Failed to write message header")?;
            ep_in.write_all(&data)
                .context("Failed to write message data")?;
            std::thread::sleep(std::time::Duration::from_secs(1));
        }
    }
}

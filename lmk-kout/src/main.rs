use std::{time::Duration, thread, fs::OpenOptions, os::unix::prelude::OpenOptionsExt};

use lmk_hid::key;

fn main() {
    thread::sleep(Duration::from_secs(1));

    let mut hid = OpenOptions::new()
        .read(true)
        .write(true)
        .open("/dev/hidg0").unwrap();
    let packets = key::string_to_packets("Hello, world!");
    key::send_key_packets(&packets, &mut hid).unwrap();
}

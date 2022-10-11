use std::{time::Duration, thread, fs::OpenOptions, io::{self, BufRead}};

use lmk_hid::key::{self, KeyPacket, KeyOrigin};

fn main() {
    thread::sleep(Duration::from_secs(1));

    let mut hid = OpenOptions::new()
        .read(true)
        .write(true)
        .open("/dev/hidg0").unwrap();

    let newline = key::string_to_packets("\n");
    let stdin = io::stdin();
    for line in stdin.lock().lines() {
        let packets = key::string_to_packets(&line.unwrap());
        key::send_key_packets(&packets, &mut hid).unwrap();
        key::send_key_packets(&newline, &mut hid).unwrap();
    }
}

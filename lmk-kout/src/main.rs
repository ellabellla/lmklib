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
    let mut add_newline = false;
    for line in stdin.lock().lines().map(|l| l.unwrap()) {
        if !add_newline {
            add_newline = true;
        } else {
            key::send_key_packets(&newline, &mut hid).unwrap();
        }
        let packets = key::string_to_packets(&line);
        key::send_key_packets(&packets, &mut hid).unwrap();
    }
}

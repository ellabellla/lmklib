use std::{time::Duration, thread, io::{self, BufRead}, str::FromStr};

use lmk_hid::{key::{Keyboard}, HID};

fn main() {
    thread::sleep(Duration::from_secs(1));

    let mut hid = HID::new(1, 0);

    let mut keyboard = Keyboard::new();
    let newline = Keyboard::from_str("\n").unwrap();
    let stdin = io::stdin();
    let mut add_newline = false;
    for line in stdin.lock().lines().map(|l| l.unwrap()) {
        if !add_newline {
            add_newline = true;
        } else {
            newline.send_keep(&mut hid).unwrap();
        }
        keyboard.press_string(&line);
        keyboard.send(&mut hid).unwrap();
    }
}

#![doc = include_str!("../README.md")]
use std::{io::{self, BufRead}, str::FromStr, fs};
use clap::{Parser};
use virt_hid::{key::{Keyboard}, HID};

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// Optional input files ('-' can be passed to mean stdio)
    inputs: Vec<String>,
}


fn kout_stdin(hid: &mut HID) {
    let mut keyboard = Keyboard::new();
    let newline = Keyboard::from_str("\n").unwrap();
    let stdin = io::stdin();
    let mut add_newline = false;
    for line in stdin.lock().lines().map(|l| l.unwrap()) {
        if !add_newline {
            add_newline = true;
        } else {
            newline.send_keep(hid).unwrap();
        }
        keyboard.press_basic_string(&line);
        keyboard.send(hid).unwrap();
    }
}

fn main() {
    let args = Cli::parse();

    let mut hid = match HID::new(1, 0) {
        Ok(hid) => hid,
        Err(_) => {
            println!("Couldn't connect to HID.");
            return
        },
    };

    if args.inputs.len() == 0 {
        kout_stdin(&mut hid);
    } else {
        let mut keyboard = Keyboard::new();
        for input in args.inputs {
            if input == "-" {
                kout_stdin(&mut hid);
            } else {
                if let Ok(contents) = fs::read_to_string(&input) {
                    keyboard.press_basic_string(&contents);
                    keyboard.send(&mut hid).unwrap();
                } else {
                    println!("Couldn't open file \"{}\"", input);
                }
            }
        }
    }
}

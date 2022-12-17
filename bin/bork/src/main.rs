#![doc = include_str!("../README.md")]

use std::fs::{self};

use clap::{Parser};
use interpreter::BorkInterp;
use virt_hid::HID;

mod parser;
mod interpreter;
#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// Input script
    input: String,

    #[cfg(feature = "debug")]
    /// Debug state data
    state: String,
}   

fn main() {
    let args = Cli::parse();

    let source = match fs::read_to_string(&args.input) {
        Ok(input) => input,
        Err(_) => {
            println!("Error, Couldn't open file {}.", &args.input);
            return
        },
    };

    let mut hid = match HID::new("/lmk/hid/mouse", "/lmk/hid/keyboard", "/lmk/hid/led") {
        Ok(hid) => hid,
        Err(_) => {
            println!("Error, Couldn't connect to HID.");
            return
        },
    };

    #[cfg(feature = "debug")]
    {
        println!("Key packet out: '{:?}'", hid.get_keyboard_path());
        println!("Mouse packet out: '{:?}'", hid.get_mouse_path());
        match hid.set_state_data(&args.state) {
            Err(e) => {
                println!("Couldn't open debug state data: {}", e);
                return;
            }
            _ => ()
        };
    }

    let interpreter = BorkInterp::new(&source);

    if let Err(e) = interpreter.run(&mut hid) {
        println!("{:?}", e);
    }
}

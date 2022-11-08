use std::fs::{self};

use clap::{Parser};
use interpreter::BorkInterp;
use lmk_hid::HID;

mod parser;
mod interpreter;
#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// Input script
    input: String,
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

    let mut hid = match HID::new(1, 0) {
        Ok(hid) => hid,
        Err(_) => {
            println!("Error, Couldn't connect to HID.");
            return
        },
    };

    let interpreter = BorkInterp::new(&source);

    if let Err(e) = interpreter.run(&mut hid) {
        println!("{:?}", e);
    }
}

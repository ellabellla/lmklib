use std::{fs};

use clap::Parser;
use interpreter::QuackInterp;
use lmk_hid::HID;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// Input script
    input: String,
}

mod parser;
mod interpreter;

fn main() {
    let args = Cli::parse();

    let input = fs::read_to_string(args.input).unwrap();


    let mut hid = HID::new(1, 0);
    let interpreter = QuackInterp::new(&input);
    interpreter.run(&mut hid);
}

#[cfg(test)]
mod tests {

    #[test]
    pub fn test() {
    }
}
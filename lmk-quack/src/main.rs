#![doc = include_str!("../README.md")]
use std::{fs};

use clap::Parser;
use interpreter::QuackInterp;
use lmk_hid::HID;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// Input script
    input: String,

    #[arg(short, long, default_value_t=false)]
    /// Halt on errors
    strict: bool,

    #[arg(short='c', long, default_value_t=false)]
    /// Hide comments
    no_comments: bool,

    #[arg(short='e', long, default_value_t=false)]
    /// Hide errors
    no_errors: bool,    
}

mod parser;
mod interpreter;

fn main() {
    let args = Cli::parse();

    let input = match fs::read_to_string(&args.input) {
        Ok(input) => input,
        Err(_) => {
            if !args.no_errors {
                println!("Error, Couldn't open file {}.", &args.input)
            }
            return
        },
    };

    let mut hid = match HID::new(1, 0) {
        Ok(hid) => hid,
        Err(_) => {
            if !args.no_errors {
                println!("Error, Couldn't connect to HID.")
            }
            return
        },
    };
    
    let interpreter = QuackInterp::new(&input);
    if let Err((line, e)) = interpreter.run(&mut hid, &!args.no_comments, &!args.no_errors, &!args.strict) {
        if !args.no_errors {
            println!("{}", e.to_err_msg(&line))
        }
    }
}

#[cfg(test)]
mod tests {

    #[test]
    pub fn test() {
    }
}
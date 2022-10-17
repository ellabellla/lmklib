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
    /// Halt on error
    strict: bool,

    #[arg(short, long, default_value_t=false)]
    /// Hide comments
    comments: bool,

    #[arg(short, long, default_value_t=false)]
    /// Hide errors
    errors: bool,    
}

mod parser;
mod interpreter;

fn main() {
    let args = Cli::parse();

    let input = fs::read_to_string(args.input).unwrap();

    let mut hid = HID::new(1, 0);
    let interpreter = QuackInterp::new(&input);
    if let Err((line, e)) = interpreter.run(&mut hid, &!args.comments, &!args.errors, &!args.strict) {
        if !args.errors {
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
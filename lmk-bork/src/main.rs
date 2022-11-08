use std::fs::{self};

use clap::{Parser};
use interpreter::BorkInterp;

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
    let source = match fs::read_to_string(&args.input){
        Ok(source) => source,
        Err(_) => {
            println!("Could not open file '{}'.", args.input);
            return;
        },
    };
    let borker = BorkInterp::new(&source);

    borker.run().unwrap();    
}

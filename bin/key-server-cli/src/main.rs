#![doc = include_str!("../README.md")]

use std::{fmt::Display, process::exit};

use clap::{Parser, Subcommand};
use key_rpc::Client;

pub trait OrExit<T> {
    fn or_exit(self, msg: &str) -> T;
}

/// Implementation for Result
impl<T, E> OrExit<T> for std::result::Result<T, E>
where
    E: Display,
{
    fn or_exit(self, msg: &str) -> T {
        match self {
            Ok(t) => t,
            Err(e) => {
                println!("{}, {}", msg, e);
                exit(1);
            }
        }
    }
}

/// Implementation for Option
impl<T> OrExit<T> for Option<T> {
    fn or_exit(self, msg: &str) -> T {
        match self {
            Some(t) => t,
            None => {
                print!("{}", msg);
                exit(1);
            }
        }
    }
}

#[derive(Parser)]
struct Args {
    #[arg(short, long)]
    ipc: Option<String>,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    Layer,
    LayerIdx,
    NumLayers,
    AddLayer { json: String },
    RemoveLayer { idx: usize },
    SwitchLayer { idx: usize },
    UpLayer,
    DownLayer,
    SaveLayout,
    Variables,
    SetVariable { name: String, data: String },
    GetVariable{ name:String },
}

fn main() {
    let args = Args::parse();
    let ipc = args.ipc.unwrap_or("ipc:///lmk/ksf.ipc".to_string());
    let mut client = Client::new(&ipc).unwrap();

    match args.command {
        Command::Layer => println!("{}", client.layer().or_exit("Unable to get layer")),
        Command::LayerIdx => println!("{}", client.layer_idx().or_exit("Unable to get layer")),
        Command::NumLayers => println!("{}", client.num_layers().or_exit("Unable to get layer")),
        Command::AddLayer { json } => client.add_layer(json).or_exit("Unable to add layer"),
        Command::RemoveLayer { idx } => client.remove_layer(idx).or_exit("Unable to add layer"),
        Command::SwitchLayer { idx } => client.switch_layer(idx).or_exit("Unable to switch layer"),
        Command::UpLayer => client.up_layer().or_exit("Unable to switch layer"),
        Command::DownLayer => client.down_layer().or_exit("Unable to switch layer"),
        Command::SaveLayout => client.save_layer().or_exit("Unable to save layout"),
        Command::Variables => println!("{:?}", client.variables().or_exit("Unable to get variables")),
        Command::SetVariable{name, data} => client
            .set_variable(name, data)
            .or_exit("Unable to set variable"),
        Command::GetVariable{name} => println!(
            "{}",
            client.get_variable(name).or_exit("Unable to get variable")
        ),
    }
}

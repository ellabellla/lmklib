#![doc = include_str!("../README.md")]

use std::{fs, io, process::Command};

use clap::{Parser, Subcommand};
use nix::unistd::Uid;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[clap(subcommand)]
    command: Commands
}

#[derive(Subcommand)]
pub enum Commands {
    /// Install the gadget service
    Install,
    /// Uninstall the gadget service
    Uninstall,
    /// Enable
    Enable,
    /// Disable
    Disable,
    /// Remove all files created during install
    Clean,
}


const KEYBOARD_DESC: &'static [u8] = include_bytes!("../keyboard.desc");
const MOUSE_DESC: &'static [u8] = include_bytes!("../mouse.desc");
const GADGET_SCHEMA: &'static str = include_str!("../gadget-schema.json");
const SERVICE: &'static str = include_str!("../gadget.service");

const SERVICE_LOC: &'static str = "/etc/systemd/system/gadget.service";
const DATA_LOC: &'static str = "/usr/gadget/";
const KEYBOARD_FILE: &'static str = "keyboard.desc";
const MOUSE_FILE: &'static str = "mouse.desc";
const GADGET_SCHEMA_FILE: &'static str = "gadget-schema.json";

const GADGET_SERVICE_INSTALL: &'static str = "systemctl daemon-reload && systemctl enable gadget.service";
const GADGET_SERVICE_UNINSTALL: &'static str = "systemctl stop gadget.service && systemctl disable gadget.service && systemctl daemon-reload";
const GADGET_SERVICE_ENABLE: &'static str = "systemctl disable gadget.service && systemctl daemon-reload";
const GADGET_SERVICE_DISABLE: &'static str = "systemctl enable gadget.service && systemctl daemon-reload";

pub fn main() {
    let args = Cli::parse();

    if !Uid::effective().is_root() {
        println!("You must run this executable with root permissions");
        return;
    }

    match args.command {
        Commands::Install => if let Err(e) = install().or_else(|e| {
                let _ = uninstall();
                Err(e)
            }) {
                println!("Install aborted due to an error, {}", e);
            },
        Commands::Uninstall => if let Err(e) = uninstall() {
            println!("Uninstall could not finish due to an error, {}", e);
        },
        Commands::Clean => if let Err(e) = clean(){
            println!("Clean could not finish due to an error, {}", e);
        },
        Commands::Enable => if let Err(e) = enable(){
            println!("The gadget service could not be enabled due to an error, {}", e);
        },
        Commands::Disable => if let Err(e) = disable(){
            println!("The gadget service could not be disabled due to an error, {}", e);
        },
    }

}

fn install() -> io::Result<()> {
    fs::create_dir_all(DATA_LOC)?;
    fs::write(DATA_LOC.to_string() + KEYBOARD_FILE, KEYBOARD_DESC)?;
    fs::write(DATA_LOC.to_string() + MOUSE_FILE, MOUSE_DESC)?;
    fs::write(DATA_LOC.to_string() + GADGET_SCHEMA_FILE, GADGET_SCHEMA)?;
    fs::write(SERVICE_LOC, SERVICE)?;

    run_command(GADGET_SERVICE_INSTALL)
}

fn uninstall() -> io::Result<()> {
    run_command(GADGET_SERVICE_UNINSTALL)?;
    clean()
}

fn enable() -> io::Result<()> {
    run_command(GADGET_SERVICE_ENABLE)
}

fn disable() -> io::Result<()> {
    run_command(GADGET_SERVICE_DISABLE)
}

fn clean() -> io::Result<()> {
    ignore_not_found(fs::remove_file(DATA_LOC.to_string() + KEYBOARD_FILE))?;
    ignore_not_found(fs::remove_file(DATA_LOC.to_string() + MOUSE_FILE))?;
    ignore_not_found(fs::remove_file(DATA_LOC.to_string() + GADGET_SCHEMA_FILE))?;
    ignore_not_found(fs::remove_file(SERVICE_LOC))
}

fn run_command(command: &str) -> io::Result<()> {
    Command::new("bash")
        .args(["-c", command])
        .spawn()
        .and_then(|mut child| child.wait())
        .map(|_| ())
}

fn ignore_not_found(res: io::Result<()>) -> io::Result<()> {
    match res {
        Ok(_) => (),
        Err(e) => if !matches!(e.kind(), io::ErrorKind::NotFound) {
            return Err(e);
        },
    }

    Ok(())
}
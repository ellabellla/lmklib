use std::{process::exit, collections::HashMap, thread, time::Duration, path::PathBuf, str::FromStr, fmt::Display, fs, io::Write};

use clap::Parser;
use driver::{DriverManager};
use function::{FunctionBuilder};

use crate::function::{HID, midi::MidiController};


mod ledstate;
mod driver;
mod layout;
mod function;

#[derive(Parser)]
struct Args {
    #[arg(short, long)]
    config: Option<String>
}

pub trait OrExit<T> {
    fn or_exit(self, msg: &str) -> T;
}

impl<T, E> OrExit<T> for std::result::Result<T, E> 
where
    E: Display
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

impl<T> OrExit<T> for Option<T> {
    fn or_exit(self, msg: &str) -> T {
        match self {
            Some(t) => t,
            None => {
                println!("{}", msg);
                exit(1);
            }
        }
    }
}

#[tokio::main]
async fn main() {
    const DRIVER_JSON: &str = "driver.json";
    const LAYOUT_JSON: &str = "layout.json";

    let args = Args::parse();

    let config = args.config
        .map(|path| {
            PathBuf::from_str(&path)
            .or_exit("Invalid config path")
        })
        .or_else(|| dirs::config_dir().map(|p| p.join("kserver")))
        .or_exit("Unable to locate config directory");
    
    if !config.exists() {
        fs::create_dir_all(&config)
            .or_exit("Unable to create config folder");
        
        fs::File::create(config.join(DRIVER_JSON))
            .or_exit("Unable to create default driver config")
            .write_all(&serde_json::to_string_pretty(&DriverManager::new(HashMap::new()))
                .or_exit("Unable to create default driver config")
                .as_bytes()
            )
            .or_exit("Unable to create default driver config");

        fs::File::create(config.join(LAYOUT_JSON))
            .or_exit("Unable to create default layout config")
            .write_all(&serde_json::to_string_pretty(&layout::LayoutBuilder::new(15, 6))
                .or_exit("Unable to create default layout config")
                .as_bytes()
            )
            .or_exit("Unable to create default layout config");
    }
    

    tokio::task::spawn_blocking(move || {
        let driver_manager: DriverManager = serde_json::from_reader(fs::File::open(config.join(DRIVER_JSON))
            .or_exit("Unable to read driver config"))
            .or_exit("Unable to parse driver config");

        let builder: layout::LayoutBuilder = serde_json::from_reader(fs::File::open(config.join(LAYOUT_JSON))
            .or_exit("Unable to read layout config"))
            .or_exit("Unable to parse layout config");

        
        let hid = HID::new(1, 0).or_exit("Unable to create hid");
        let midi_controller = MidiController::new().or_exit("Unable to create midi controller");
        let func_builder = FunctionBuilder::new(hid, midi_controller);

        let mut layout = builder.build(driver_manager, &func_builder);

        loop {
            layout.tick();
            layout.poll();
            thread::sleep(Duration::from_millis(2));
        }
    }).await.unwrap();

    // let mut builder = MCP23017DriverBuilder::new("mcp1", 0x20, 3);
    // let x = vec![Pin::new(14).or_exit("pin"), Pin::new(11).or_exit("pin"), Pin::new(8).or_exit("pin")];
    // let y = vec![Pin::new(15).or_exit("pin"), Pin::new(13).or_exit("pin"), Pin::new(9).or_exit("pin")];
    // let matrix = builder.add_matrix(x, y).or_exit("add");
    // let mut mcp = builder.build().or_ex

    // loop {
    //     mcp.tick();        
    //     let state = mcp.poll_range(&matrix).or_exit("state");
    //     for (i, state) in state.iter().enumerate() {
    //         print!("{} ", state);
    //         if i != 0 && (i + 1)  %3 == 0 {
    //             println!()
    //         }
    //     }
    //     sleep(Duration::from_millis(40));
    //     print!("\x1B[2J\x1B[1;1H");

    // }
    //let mut mcp = MCP23017::new(0x20, 3).unwrap();

    // let out_y1 = Pin::new(15).unwrap();
    // mcp.pin_mode(&out_y1, Mode::Output).unwrap();
    // mcp.output(&out_y1, State::High).unwrap();
    
    // let out_y2 = Pin::new(13).unwrap();
    // mcp.pin_mode(&out_y2, Mode::Output).unwrap();
    // mcp.output(&out_y2, State::High).unwrap();
    
    // let out_y3 = Pin::new(9).unwrap();
    // mcp.pin_mode(&out_y3, Mode::Output).unwrap();
    // mcp.output(&out_y3, State::High).unwrap();

    // let in_x1 = Pin::new(14).unwrap();
    // mcp.pin_mode(&in_x1, Mode::Input).unwrap();
    // let in_x2 = Pin::new(11).unwrap();
    // mcp.pin_mode(&in_x2, Mode::Input).unwrap();
    // let in_x3 = Pin::new(8).unwrap();
    // mcp.pin_mode(&in_x3, Mode::Input).unwrap();
    // const KEYBOARD_ID: u8 = 0;

    // let led_state = LEDStateInterface::new(KEYBOARD_ID)
    //     .or_exit("Unable to open keyboard hid connection");

    // let mount = Mount::new();
    // {
    //     let mut mount = mount.write().await;
    //     mount.mount("/led", LEDStateInterface::into_configuration(&led_state));
    // }

    // let fs = FS::mount("KServer ConfigFS", "", mount.clone()).await
    //     .or_exit("Unable to mount configfs");

    // fs.await
    //     .unwrap()
    //     .unwrap();
}
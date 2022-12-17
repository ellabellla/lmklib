use std::{process::exit, thread, time::Duration, path::PathBuf, str::FromStr, fmt::Display, fs, io::Write, sync::Arc};

use clap::Parser;
use driver::{DriverManager};
use function::{FunctionBuilder};
use log::{error, LevelFilter, info};
use simplelog::{CombinedLogger, TermLogger, Config, TerminalMode, ColorChoice};
use tokio::sync::RwLock;

use crate::{function::{midi::MidiController, cmd::CommandPool, hid::HID, FunctionConfiguration, FunctionConfig, nng::NanoMessenger}, driver::SerdeDriverManager};

mod driver;
mod layout;
mod function;

#[derive(Parser)]
struct Args {
    #[arg(short, long)]
    config: Option<String>
}

pub trait OrLog<T> {
    fn or_log(self, msg: &str) -> Option<T>;
}
pub trait OrLogIgnore<T> {
    fn or_log_ignore(self, msg: &str) -> Option<T>;
}

impl<T, E> OrLog<T> for std::result::Result<T, E> 
where
    E: Display
{
    fn or_log(self, msg: &str) -> Option<T> {
        match self {
            Ok(t) => Some(t),
            Err(e) => {
                error!("{}, {}", msg, e);
                None
            }
        }
    }
}

impl<T, E> OrLogIgnore<T> for std::result::Result<T, E> {
    fn or_log_ignore(self, msg: &str) -> Option<T> {
        match self {
            Ok(t) => Some(t),
            Err(_) => {
                error!("{}", msg);
                None
            }
        }
    }
}

impl<T> OrLogIgnore<T> for Option<T> {
    fn or_log_ignore(self, msg: &str) -> Option<T> {
        match self {
            Some(t) => Some(t),
            None => {
                error!("{}", msg);
                None
            }
        }
    }
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
                error!("{}, {}", msg, e);
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
                error!("{}", msg);
                exit(1);
            }
        }
    }
}

#[tokio::main]
async fn main() {
    const BACKEND_JSON: &str = "backend.json";
    const LAYOUT_JSON: &str = "layout.json";
    const FRONTEND_JSON: &str = "frontend.json";
    
    CombinedLogger::init(
        vec![
            TermLogger::new(LevelFilter::Info, Config::default(), TerminalMode::Stdout, ColorChoice::Auto),
        ]
    ).unwrap();

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
        
        fs::File::create(config.join(BACKEND_JSON))
            .or_exit("Unable to create default backend config")
            .write_all(&serde_json::to_string_pretty(&SerdeDriverManager::new())
                .or_exit("Unable to create default backend config")
                .as_bytes()
            )
            .or_exit("Unable to create default backend config");

        fs::File::create(config.join(LAYOUT_JSON))
            .or_exit("Unable to create default layout config")
            .write_all(&serde_json::to_string_pretty(&layout::LayoutBuilder::new(15, 6))
                .or_exit("Unable to create default layout config")
                .as_bytes()
            )
            .or_exit("Unable to create default layout config");
        
        fs::File::create(config.join(FRONTEND_JSON))
            .or_exit("Unable to create default frontend config")
            .write_all(&serde_json::to_string_pretty(&FunctionConfiguration::new())
                .or_exit("Unable to create default frontend config")
                .as_bytes()
            )
            .or_exit("Unable to create default frontend config");
    }


    let driver_manager: SerdeDriverManager = serde_json::from_reader(fs::File::open(config.join(BACKEND_JSON))
        .or_exit("Unable to read backend config"))
        .or_exit("Unable to parse backend config");
    let driver_manager: Arc<RwLock<DriverManager>> = Arc::new(RwLock::new(driver_manager.build().await.or_exit("Unable to build driver manager")));
    
    let function_config: FunctionConfiguration = serde_json::from_reader(fs::File::open(config.join(FRONTEND_JSON))
        .or_exit("Unable to read frontend config"))
        .or_exit("Unable to parse frontend config");

    let command_pool = CommandPool::from_config(&function_config).await.or_exit("Unable to create command pool");
    let hid = HID::from_config(&function_config).await.or_exit("Unable to create hid");
    let nano_messanger = NanoMessenger::from_config(&function_config).await.or_exit("Unable to create nano messange");
    let midi_controller = MidiController::from_config(&function_config).await.or_exit("Unable to create midi controller");
    let func_builder = FunctionBuilder::new(hid, midi_controller, command_pool, driver_manager.clone(), nano_messanger);

    let builder: layout::LayoutBuilder = serde_json::from_reader(fs::File::open(config.join(LAYOUT_JSON))
        .or_exit("Unable to read layout config"))
        .or_exit("Unable to parse layout config");

    let mut layout = builder.build(driver_manager, &func_builder).await;

    info!("Layout:\n{}", layout.layout_string());

    tokio::spawn(async move {
        loop {
            layout.tick().await;
            layout.poll().await;
            thread::sleep(Duration::from_millis(30));
        }
    }).await.unwrap();
}
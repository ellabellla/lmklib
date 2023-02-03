use std::{process::exit, thread, time::Duration, path::PathBuf, str::FromStr, fmt::Display, fs, io::Write, sync::Arc};

use clap::Parser;
use driver::{DriverManager};
use function::{FunctionBuilder};
use log::{error};
use tokio::sync::RwLock;

use crate::{function::{midi::MidiController, cmd::CommandPool, hid::HID, FunctionConfiguration, FunctionConfig, nng::NanoMessenger}, modules::ModuleManager, config_rpc::ConfigRPC, layout::Variables};

/// Driver module
mod driver;
/// Layout module
mod layout;
/// Function module
mod function;
/// Plugin modules
mod modules;
/// Config rpc modules
mod config_rpc;

#[derive(Parser)]
/// Cli Args
struct Args {
    #[arg(short, long)]
    /// Path to config directory
    config: Option<String>
}

/// Turns a result into a option containing the ok value. 
/// If the result is an error it will log a message followed by the error message as an error.
pub trait OrLog<T> {
    fn or_log(self, msg: &str) -> Option<T>;
}

/// Accepts a result or option. If a result it is turned into a option containing the ok value. 
/// If the option is none or the result is err then it will log the message as an error.
pub trait OrLogIgnore<T> {
    fn or_log_ignore(self, msg: &str) -> Option<T>;
}


/// Implementation for Result
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

/// Implementation for Result
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

/// Implementation for Option
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

/// Accepts a result or option. If a result it is turned into a option containing the ok value. 
/// If the option is none or the result is err then it will log the message as an error, 
/// followed by the error message (for result), and exit the program with and exit status of 1.
pub trait OrExit<T> {
    fn or_exit(self, msg: &str) -> T;

    fn or_exit_print(self, msg: &str) -> T;
}

/// Implementation for Result
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

    fn or_exit_print(self, msg: &str) -> T {
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
                error!("{}", msg);
                exit(1);
            }
        }
    }

    fn or_exit_print(self, msg: &str) -> T {
        match self {
            Some(t) => t,
            None => {
                print!("{}", msg);
                exit(1);
            }
        }
    }
}

#[tokio::main]
async fn main() {
    /// Config files and folders
    const DRIVERS: &str = "drivers";
    const LAYOUT_JSON: &str = "layout.json";
    const FRONTEND_JSON: &str = "frontend.json";
    const MODULES: &str = "modules";

    const CONFIG_FRONT: &str = "ipc:///lmk/ksf.ipc";
    const CONFIG_BACK: &str = "ipc:///lmk/ksb.ipc";

    // Load configuration
    let args = Args::parse();

    let config = args.config
        .map(|path| {
            PathBuf::from_str(&path)
            .or_exit("Invalid config path")
        })
        .or_else(|| dirs::config_dir().map(|p| p.join("key-server")))
        .or_exit_print("Unable to locate config directory");

    // init logger
    let logger_config = config.join("config.yaml");
    if !logger_config.exists() {
        const DEFAULT_CONFIG: &'static str = include_str!("../log-config.yaml");
        fs::File::create(&logger_config)
            .or_exit_print("Unable to create logger config")
            .write(DEFAULT_CONFIG.as_bytes())
            .or_exit_print("Unable to create logger config");
    }
    match log4rs::init_file(&logger_config, Default::default()) {
        Ok(_) => (),
        Err(e) => {println!("unable to load logger config, {}", e); return},
    };
    
    // Load configuration
    if !config.exists() {
        fs::create_dir_all(&config)
            .or_exit("Unable to create config folder");
        
        fs::create_dir_all(config.join(DRIVERS))
            .or_exit("Unable to create driver folder");

        fs::File::create(config.join(LAYOUT_JSON))
            .or_exit("Unable to create default layout config")
            .write_all(&serde_json::to_string_pretty(&layout::LayoutBuilder::new(15, 6))
                .or_exit("Unable to create default layout config")
                .as_bytes()
            )
            .or_exit("Unable to create default layout config");
        
        fs::File::create(config.join(FRONTEND_JSON))
            .or_exit("Unable to create default frontend config")
            .write_all(FunctionConfiguration::create_config()
                .or_exit("Unable to create default frontend config")
                .as_bytes()
            )
            .or_exit("Unable to create default frontend config");
        
        fs::create_dir(config.join(MODULES))
            .or_exit("Unable to create modules folder");
    }

    // init key-server
    let module_manager = ModuleManager::new(config.join(MODULES)).or_exit("Unable to create module manager");

    let driver_manager = DriverManager::load(&config.join(DRIVERS), module_manager.clone()).await
        .or_exit("Unable to load drivers");
    let driver_manager: Arc<RwLock<DriverManager>> = Arc::new(RwLock::new(driver_manager));
    
    let function_config: FunctionConfiguration = FunctionConfiguration::new(&fs::read_to_string(config.join(FRONTEND_JSON))
        .or_exit("Unable to read frontend config"), module_manager.clone())
        .or_exit("Unable to parse frontend config");

    let command_pool = CommandPool::from_config(&function_config).await.or_exit("Unable to create command pool");
    let hid = HID::from_config(&function_config).await.or_exit("Unable to create hid");
    let nano_messanger = NanoMessenger::from_config(&function_config).await.or_exit("Unable to create nano messange");
    let midi_controller = MidiController::from_config(&function_config).await.or_exit("Unable to create midi controller");
    let variables = Variables::new();
    let func_builder = FunctionBuilder::new(
        hid, 
        midi_controller, 
        command_pool, 
        driver_manager.clone(), 
        nano_messanger, 
        module_manager.clone(),
        variables
    );

    let builder: layout::LayoutBuilder = serde_json::from_reader(fs::File::open(config.join(LAYOUT_JSON))
        .or_exit("Unable to read layout config"))
        .or_exit("Unable to parse layout config");

    let layout = builder.build(driver_manager, func_builder.clone()).await;

    let config_thread = ConfigRPC::start(
        CONFIG_FRONT.to_string(), 
        CONFIG_BACK.to_string(), 
        layout.clone(), 
        config.join(LAYOUT_JSON)
    ).await.or_exit("Unable to start Config RPC");

    // event loop
    let layout_thread = {
        let layout = layout.clone();
        tokio::spawn(async move {
            loop {
                layout.write().await.tick().await;
                layout.write().await.poll().await;
                thread::sleep(Duration::from_millis(10));
            }
        })
    };

    layout_thread.await.or_log("Layout thread error");
    config_thread.await.or_log("Config FS thread error");
}
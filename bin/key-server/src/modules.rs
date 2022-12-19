use std::{collections::HashMap, io, path::PathBuf, fs, fmt::Display, sync::Arc, ops::Range};

use abi_stable::{std_types::{RString, RVec}, library::LibraryError};
use configfs::async_trait;
use key_module::{Data, function, driver};
use serde::{Serialize, Deserialize};
use tokio::sync::{mpsc::{self, UnboundedSender}, oneshot::{self, Sender, Receiver}};

use crate::{OrLogIgnore, function::{FunctionInterface, ReturnCommand, FunctionType, Function}, OrLog, driver::{DriverInterface, DriverType, Driver, DriverError}};

#[derive(Serialize, Deserialize)]
enum ModuleType {
    Function,
    Driver,
}

#[derive(Serialize, Deserialize)]
struct Module {
    name: String,
    mod_type: ModuleType,
}

#[derive(Debug)]
pub enum ModError {
    IO(io::Error),
    LoadLibrary(LibraryError),
    Parse(serde_json::Error),
    NoSuchModule,
    Library(RString),
    Channel(String),
}

impl Display for ModError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ModError::IO(e) => f.write_fmt(format_args!("IO error, {}", e)),
            ModError::LoadLibrary(e) => f.write_fmt(format_args!("Library loading error, {}", e)),
            ModError::Parse(e) => f.write_fmt(format_args!("Parse error, {}", e)),
            ModError::NoSuchModule => f.write_str("No such module"),
            ModError::Library(e) => f.write_fmt(format_args!("Library error, {}", e)),
            ModError::Channel(e) => f.write_fmt(format_args!("Channel error, {}", e)),
        }
    }
}

pub struct ExternalFunction {
    module_name: String,
    id: u64,
    func: Data,
    module_manager: Arc<ModuleManager>,
}

impl ExternalFunction {
    pub async fn new(module_name: String, module_manager: Arc<ModuleManager>, func: Data) -> Function {
        let id = module_manager.load_function(&module_name, func.clone()).await.or_log("Load function error (External Function)")?;
        Some(Box::new(ExternalFunction{module_name, id, func, module_manager}))
    }
}

#[async_trait]
impl FunctionInterface for ExternalFunction {
    async fn event(&mut self, state: u16) -> ReturnCommand {
        self.module_manager.function_event(&self.module_name, self.id, state).await.or_log("Event error (External Function)");
        ReturnCommand::None
    }
    fn ftype(&self) -> FunctionType {
        FunctionType::External { module: self.module_name.clone(), func: self.func.clone() }
    }
}  

pub struct ExternalDriver {
    module_name: String,
    name: String,
    id: u64,
    driver: Data,
    module_manager: Arc<ModuleManager>,
    state: Vec<u16>,
}

impl ExternalDriver {
    pub async fn new(module_name: String, driver: Data, module_manager: Arc<ModuleManager>) -> Result<Driver, DriverError> {
        let id = module_manager.load_driver(&module_name, driver.clone()).await
            .map_err(|e| DriverError::new(format!("{}", e)))?;
        let name = module_manager.driver_name(&module_name, id).await
            .map_err(|e| DriverError::new(format!("{}", e)))?;
        let state = module_manager.driver_poll(&module_name, id).await
            .map_err(|e| DriverError::new(format!("{}", e)))?;
        
        Ok(Box::new(ExternalDriver{module_name, name: name.into(), id, driver, module_manager, state: state.into()}))
    }
}

#[async_trait]
impl DriverInterface for ExternalDriver {
    fn name(&self) -> &str {
        &self.name
    }

    fn iter(&self) -> std::slice::Iter<u16> {
        self.state.iter()
    }

    fn poll(&self, idx: usize) -> u16 {
        self.state.get(idx).map(|state| *state).unwrap_or(0)
    }

    fn poll_range(&self, range: &Range<usize>) -> Option<&[u16]> {
        self.state.get(range.clone())
    }

    async fn tick(&mut self) {
        if let Some(state) = self.module_manager
            .driver_poll(&self.module_name, self.id).await
            .or_log("Poll error (External Driver)") {
                self.state = state.into();
        }
    }
    
    fn to_driver_type(&self) -> DriverType {
        DriverType::External { module: self.module_name.clone(), driver: self.driver.clone() }
    }
}

enum FuncCommand {
    LoadData(Data, Sender<Result<u64, RString>>),
    Event(u64, u16, Sender<Result<(), RString>>),
}

enum DriverCommand {
    LoadData(Data, Sender<Result<u64, RString>>),
    Name(u64, Sender<Result<RString, RString>>),
    Poll(u64, Sender<Result<RVec<u16>, RString>>),
}

pub struct ModuleManager {
    function_modules: HashMap<String, UnboundedSender<FuncCommand>>,
    driver_modules: HashMap<String, UnboundedSender<DriverCommand>>,
}

impl ModuleManager {
    pub fn new(plugin_dir: PathBuf) -> Result<Arc<ModuleManager>, ModError> {
        let contents = fs::read_dir(plugin_dir).map_err(|e| ModError::IO(e))?;
        let mut modules = ModuleManager{function_modules: HashMap::new(), driver_modules: HashMap::new()};
        
        for entry in contents {
            let entry = entry.map_err(|e| ModError::IO(e))?;
            modules.load_module(entry.path())?;
        }

        Ok(Arc::new(modules))
    }

    fn load_module(&mut self, module_path: PathBuf) -> Result<(), ModError> {
        let meta = module_path.join("meta.json");
        let module: Module = serde_json::from_reader(fs::File::open(meta)
            .map_err(|e| ModError::IO(e))?)
            .map_err(|e| ModError::Parse(e))?;
        
        match module.mod_type {
            ModuleType::Function => self.load_function_module(module_path, module)?,
            ModuleType::Driver => self.load_driver_module(module_path, module)?,
        }
        
        Ok(())
    }

    fn load_function_module(&mut self, module_path: PathBuf, module: Module) -> Result<(), ModError> {
        let interface = function::load_module(&module_path.join("module.so"))
            .map_err(|e| ModError::LoadLibrary(e))?;

        let (tx, mut rx) = mpsc::unbounded_channel();
        tokio::task::spawn_blocking(move || {
            let mut func = interface.new_function()();
            while let Some(command) = rx.blocking_recv() {
                match command {
                    FuncCommand::LoadData(data, tx) => tx.send(func.load_data(data).into())
                        .or_log_ignore("Channel error (Modules)"),
                    FuncCommand::Event(id, state, tx) => tx.send(func.event(id, state).into())
                        .or_log_ignore("Channel error (Modules)"),
                };
            }
        });

        self.function_modules.insert(module.name, tx);
        Ok(())
    }

    fn load_driver_module(&mut self, module_path: PathBuf, module: Module) -> Result<(), ModError> {
        let interface = driver::load_module(&module_path.join("module.so"))
            .map_err(|e| ModError::LoadLibrary(e))?;

        let (tx, mut rx) = mpsc::unbounded_channel();
        tokio::task::spawn_blocking(move || {
            let mut driver = interface.new_driver()();
            while let Some(command) = rx.blocking_recv() {
                match command {
                    DriverCommand::LoadData(data, tx) => tx.send(driver.load_data(data).into())
                        .or_log_ignore("Channel error (Modules)"),
                    DriverCommand::Name(id, tx) => tx.send(driver.name(id).into())
                        .or_log_ignore("Channel error (Modules)"),
                    DriverCommand::Poll(id, tx) => tx.send(driver.poll(id).into())
                        .or_log_ignore("Channel error (Modules)"),
                };
            }
        });

        self.driver_modules.insert(module.name, tx);
        Ok(())
    }

    fn find_function_module(&self, module_name: &str) -> Result<&UnboundedSender<FuncCommand>, ModError> {
        self.function_modules.get(module_name).ok_or_else(|| ModError::NoSuchModule)
    }

    fn find_driver_module(&self, module_name: &str) -> Result<&UnboundedSender<DriverCommand>, ModError> {
        self.driver_modules.get(module_name).ok_or_else(|| ModError::NoSuchModule)
    }

    async fn receive<T>(rx: Receiver<Result<T, RString>>) -> Result<T, ModError>{
        match rx.await {
            Ok(res) => res.map_err(|e| ModError::Library(e)),
            Err(e) => Err(ModError::Channel(e.to_string())),
        }
    } 

    pub async fn load_function(&self, module_name: &str, data: Data) -> Result<u64, ModError> {
        let module = self.find_function_module(module_name)?;
        let (tx, rx) = oneshot::channel();
        module.send(FuncCommand::LoadData(data, tx)).map_err(|e| ModError::Channel(e.to_string()))?;
        ModuleManager::receive(rx).await
    }

    pub async fn function_event(&self, module_name: &str, id: u64, state: u16) -> Result<(), ModError> {
        let module = self.find_function_module(module_name)?;
        let (tx, rx) = oneshot::channel();
        module.send(FuncCommand::Event(id, state, tx)).map_err(|e| ModError::Channel(e.to_string()))?;
        ModuleManager::receive(rx).await
    }

    pub async fn load_driver(&self, module_name: &str, data: Data) -> Result<u64, ModError> {
        let module = self.find_driver_module(module_name)?;
        let (tx, rx) = oneshot::channel();
        module.send(DriverCommand::LoadData(data, tx)).map_err(|e| ModError::Channel(e.to_string()))?;
        ModuleManager::receive(rx).await
    }

    pub async fn driver_name(&self, module_name: &str, id: u64) -> Result<RString, ModError> {
        let module = self.find_driver_module(module_name)?;
        let (tx, rx) = oneshot::channel();
        module.send(DriverCommand::Name(id, tx)).map_err(|e| ModError::Channel(e.to_string()))?;
        ModuleManager::receive(rx).await
    }

    pub async fn driver_poll(&self, module_name: &str, id: u64) -> Result<RVec<u16>, ModError> {
        let module = self.find_driver_module(module_name)?;
        let (tx, rx) = oneshot::channel();
        module.send(DriverCommand::Poll(id, tx)).map_err(|e| ModError::Channel(e.to_string()))?;
        ModuleManager::receive(rx).await
    }


}
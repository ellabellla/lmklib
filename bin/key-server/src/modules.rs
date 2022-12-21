use std::{collections::HashMap, io, path::PathBuf, fs, fmt::Display, sync::Arc, ops::{Range, Deref, DerefMut}};

use pyo3::{prelude::*};
use configfs::async_trait;
use key_module::{Data, function, driver};
use serde::{Serialize, Deserialize};
use tokio::sync::{mpsc::{self, UnboundedSender}, oneshot::{self, Sender, Receiver}};

use crate::{OrLogIgnore, function::{FunctionInterface, ReturnCommand, FunctionType, Function}, OrLog, driver::{DriverInterface, DriverType, Driver, DriverError}};

#[derive(Debug, Serialize, Deserialize)]
/// Interface type
enum InterfaceType {
    /// Function interface
    Function,
    /// Driver interface
    Driver,
}

#[derive(Debug, Serialize, Deserialize)]
/// Module type
enum ModuleType {
    /// abi_stable rust module
    ABIStable,
    /// python module
    Python,
}

#[derive(Debug, Serialize, Deserialize)]
/// Module meta data
struct Module {
    /// Name of module
    name: String,
    /// Interface type
    interface: InterfaceType,
    /// Module type
    module_type: ModuleType,
}

#[derive(Debug)]
/// Module Error
pub enum ModError {
    /// IO
    IO(io::Error),
    /// Error loading module
    LoadModule(String),
    /// Meta parse error
    Parse(serde_json::Error),
    /// No module found
    NoSuchModule,
    /// Internal module error
    Module(String),
    /// Message passing error
    Channel(String),
}

impl Display for ModError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ModError::IO(e) => f.write_fmt(format_args!("IO error, {}", e)),
            ModError::LoadModule(e) => f.write_fmt(format_args!("Module loading error, {}", e)),
            ModError::Parse(e) => f.write_fmt(format_args!("Parse error, {}", e)),
            ModError::NoSuchModule => f.write_str("No such module"),
            ModError::Module(e) => f.write_fmt(format_args!("Module error, {}", e)),
            ModError::Channel(e) => f.write_fmt(format_args!("Channel error, {}", e)),
        }
    }
}

/// Internal "External Function" Interface
pub struct ExternalFunction {
    module_name: String,
    id: u64,
    func: Data,
    module_manager: Arc<ModuleManager>,
}

impl ExternalFunction {
    /// New
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

/// Internal "External Driver" Interface
pub struct ExternalDriver {
    module_name: String,
    name: String,
    id: u64,
    driver: Data,
    module_manager: Arc<ModuleManager>,
    state: Vec<u16>,
}

impl ExternalDriver {
    /// New
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

#[derive(Debug)]
/// Function Module commands
enum FuncCommand {
    /// Load and init new driver from data
    LoadData(Data, Sender<Result<u64, String>>),
    /// State poll event
    Event(u64, u16, Sender<Result<(), String>>),
}

#[derive(Debug)]
/// Driver Module commands
enum DriverCommand {
    /// Load and init new driver from data
    LoadData(Data, Sender<Result<u64, String>>),
    /// Get name of driver
    Name(u64, Sender<Result<String, String>>),
    /// Poll the state of the driver
    Poll(u64, Sender<Result<Vec<u16>, String>>),
}

#[derive(FromPyObject)]
/// Rust representation of explicit Python result object
struct Pyo3Result<T, E> {
    #[pyo3(item)]
    ok: Option<T>,
    #[pyo3(item)]
    err: Option<E>,
}

/// Rust representation of lose Python result. Stores the data as a result that can be used.
struct WrapResult<T, E>(Result<T, E>);

impl<'source, T, E> FromPyObject<'source> for WrapResult<T, E> 
where
    T: FromPyObject<'source>,
    E: FromPyObject<'source>,
{
    fn extract(ob: &'source PyAny) -> PyResult<Self> {
        if let Ok(res) = ob.extract::<Pyo3Result<T, E>>() {
            if let Some(res) = res.ok {
                return Ok(WrapResult(Ok(res)))
            } else if let Some(res) = res.err {
                return Ok(WrapResult(Err(res)))
            }
        }
        
        if let Ok(res) = ob.extract() {
            Ok(WrapResult(Ok(res)))
        } else {
            let res = ob.extract()?;
            return Ok(WrapResult(Err(res)))
        }
    }
} 

impl<T, E> Deref for WrapResult<T, E> {
    type Target = Result<T, E>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T, E> DerefMut for WrapResult<T, E> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<T, E> Into<Result<T, E>> for WrapResult<T, E> {
    fn into(self) -> Result<T, E> {
        self.0
    }
}

/// Module Manager
pub struct ModuleManager {
    function_modules: HashMap<String, UnboundedSender<FuncCommand>>,
    driver_modules: HashMap<String, UnboundedSender<DriverCommand>>,
}

/// Module meta file name
const META_FILE: &'static str = "meta.json";
/// ABI Module file name
const ABI_MODULE_FILE: &'static str = "module.so";
/// Python module file name
const PY_MODULE_FILE: &'static str = "module.py";

/// Python load data function name
const PY_LOAD_DATA: &'static str = "load_data";
/// Python load data function name
const PY_EVENT: &'static str = "event";
/// Python name function name
const PY_NAME: &'static str = "name";
/// Python poll function name
const PY_POLL: &'static str = "poll";

impl ModuleManager {
    /// New
    pub fn new(plugin_dir: PathBuf) -> Result<Arc<ModuleManager>, ModError> {
        let contents = fs::read_dir(plugin_dir).map_err(|e| ModError::IO(e))?;
        let mut modules = ModuleManager{function_modules: HashMap::new(), driver_modules: HashMap::new()};
        
        for entry in contents {
            let entry = entry.map_err(|e| ModError::IO(e))?;
            modules.load_module(entry.path())?;
        }

        Ok(Arc::new(modules))
    }

    /// Load module
    fn load_module(&mut self, module_path: PathBuf) -> Result<(), ModError> {
        let meta = module_path.join(META_FILE);
        let module: Module = serde_json::from_reader(fs::File::open(meta)
            .map_err(|e| ModError::IO(e))?)
            .map_err(|e| ModError::Parse(e))?;
        
        match module.interface {
            InterfaceType::Function => self.load_function_module(module_path, module)?,
            InterfaceType::Driver => self.load_driver_module(module_path, module)?,
        }
        
        Ok(())
    }

    /// Load function module
    fn load_function_module(&mut self, module_path: PathBuf, module: Module) -> Result<(), ModError> {
        let tx = match module.module_type {
            ModuleType::ABIStable => ModuleManager::init_abi_function(module_path)?,
            ModuleType::Python => ModuleManager::init_py_function(module_path)?,
        };

        self.function_modules.insert(module.name, tx);
        Ok(())
    }

    /// Init abi function
    fn init_abi_function(module_path: PathBuf) -> Result<UnboundedSender<FuncCommand>, ModError> {
        let interface = function::load_module(&module_path.join(ABI_MODULE_FILE))
            .map_err(|e| ModError::LoadModule(e.to_string()))?;

        let (tx, mut rx) = mpsc::unbounded_channel();
        tokio::task::spawn_blocking(move || {
            let mut func = interface.new_function()();
            while let Some(command) = rx.blocking_recv() {
                match command {
                    FuncCommand::LoadData(data, tx) => tx.send(
                            func.load_data(data)
                            .map_err(|e| e.into_string())
                            .into()
                        )
                        .or_log_ignore("Channel error (Module Manager)"),
                    FuncCommand::Event(id, state, tx) => tx.send(
                            func.event(id, state)
                            .map_err(|e| e.into_string())
                            .into()
                        )
                        .or_log_ignore("Channel error (Module Manager)"),
                };
            }
        });
        Ok(tx)
    }

    /// Init python function
    fn init_py_function(module_path: PathBuf) -> Result<UnboundedSender<FuncCommand>, ModError> {
        let path = module_path.join(PY_MODULE_FILE);
        let path_str = path.to_string_lossy().to_string();
        let code = fs::read_to_string(&path)
            .map_err(|e|ModError::LoadModule(e.to_string()))?;
        let interface = Python::with_gil(|py| -> PyResult<Py<PyAny>> {
            Ok(PyModule::from_code(py, &code, &path_str, &path_str)?.into())
        }).map_err(|e| ModError::LoadModule(e.to_string()))?;

        let (tx, mut rx) = mpsc::unbounded_channel();
        tokio::task::spawn_blocking(move || {
            while let Some(command) = rx.blocking_recv() {
                Python::with_gil(|py| -> PyResult<Py<PyAny>> {
                    match command {
                        FuncCommand::LoadData(data, tx) => {
                            let res = || -> Result<_, PyErr> { 
                                Ok(interface.getattr(py, PY_LOAD_DATA)?
                                .call1(py, (data.name.into_string(), data.data.into_string()))?
                                .extract::<WrapResult<_, _>>(py)?.0)
                            };
                            match res() {
                                Ok(res) => tx.send(res).or_log_ignore("Channel error (Module Manager)"),
                                Err(e) => tx.send(Err(e.to_string())).or_log_ignore("Channel error (Module Manager)"),
                            }
                        },
                        FuncCommand::Event(id, state, tx) => {
                            let res = || -> Result<_, PyErr> { 
                                Ok(interface.getattr(py, PY_EVENT)?
                                .call1(py, (id, state))?
                                .extract::<Option<_>>(py)?)
                            };
                            match res() {
                                Ok(Some(e)) => tx.send(Err(e)).or_log_ignore("Channel error (Module Manager)"),
                                Ok(None) => tx.send(Ok(())).or_log_ignore("Channel error (Module Manager)"),
                                Err(e) => tx.send(Err(e.to_string())).or_log_ignore("Channel error (Module Manager)"),
                            }
                        }
                    };
                    Ok(py.None())
                }).map_err(|e| ModError::LoadModule(e.to_string()))
                    .or_log("Python error (Module Manager)");
            }
        });
        Ok(tx)
    }

    /// Load driver module
    fn load_driver_module(&mut self, module_path: PathBuf, module: Module) -> Result<(), ModError> {
        let tx = match module.module_type {
            ModuleType::ABIStable => ModuleManager::init_abi_driver(module_path)?,
            ModuleType::Python => ModuleManager::init_py_driver(module_path)?,
        };

        self.driver_modules.insert(module.name, tx);
        Ok(())
    }

    /// Init abi driver
    fn init_abi_driver(module_path: PathBuf) -> Result<UnboundedSender<DriverCommand>, ModError> {
        let interface = driver::load_module(&module_path.join(ABI_MODULE_FILE))
            .map_err(|e| ModError::LoadModule(e.to_string()))?;

        let (tx, mut rx) = mpsc::unbounded_channel();
        tokio::task::spawn_blocking(move || {
            let mut driver = interface.new_driver()();
            while let Some(command) = rx.blocking_recv() {
                match command {
                    DriverCommand::LoadData(data, tx) => tx.send(
                            driver.load_data(data)
                            .map_err(|e| e.to_string())
                            .into()
                        )
                        .or_log_ignore("Channel error (Module Manager)"),
                    DriverCommand::Name(id, tx) => tx.send(
                            driver.name(id)
                            .map(|o| o.to_string())
                            .map_err(|e| e.to_string())
                            .into()
                        )
                        .or_log_ignore("Channel error (Module Manager)"),
                    DriverCommand::Poll(id, tx) => tx.send(
                            driver.poll(id)
                            .map(|o| o.into())
                            .map_err(|e| e.to_string())
                            .into()
                        )
                        .or_log_ignore("Channel error (Module Manager)"),
                };
            }
        });
        Ok(tx)
    }

    /// Init python driver
    fn init_py_driver(module_path: PathBuf) -> Result<UnboundedSender<DriverCommand>, ModError> {
        let path = module_path.join(PY_MODULE_FILE);
        let path_str = path.to_string_lossy().to_string();
        let code = fs::read_to_string(&path)
            .map_err(|e|ModError::LoadModule(e.to_string()))?;
        let interface = Python::with_gil(|py| -> PyResult<Py<PyAny>> {
            Ok(PyModule::from_code(py, &code, &path_str, &path_str)?.into())
        }).map_err(|e| ModError::LoadModule(e.to_string()))?;

        let (tx, mut rx) = mpsc::unbounded_channel();
        tokio::task::spawn_blocking(move || {
            while let Some(command) = rx.blocking_recv() {
                Python::with_gil(|py| -> PyResult<Py<PyAny>> {
                    match command {
                        DriverCommand::LoadData(data, tx) => {
                            let res = || -> Result<_, PyErr> { 
                                Ok(interface.getattr(py, PY_LOAD_DATA)?
                                .call1(py, (data.name.into_string(), data.data.into_string()))?
                                .extract::<WrapResult<_, _>>(py)?.0)
                            };
                            match res() {
                                Ok(res) => tx.send(res).or_log_ignore("Channel error (Module Manager)"),
                                Err(e) => tx.send(Err(e.to_string())).or_log_ignore("Channel error (Module Manager)"),
                            }
                        },
                        DriverCommand::Name(id, tx) => {
                            let res = || -> Result<_, PyErr> { 
                                Ok(interface.getattr(py, PY_NAME)?
                                .call1(py, (id as i64,))?
                                .extract::<WrapResult<_, _>>(py)?.0)
                            };
                            match res() {
                                Ok(res) => tx.send(res).or_log_ignore("Channel error (Module Manager)"),
                                Err(e) => tx.send(Err(e.to_string())).or_log_ignore("Channel error (Module Manager)"),
                            }
                        },
                        DriverCommand::Poll(id, tx) => {
                            let res = || -> Result<_, PyErr> { 
                                Ok(interface.getattr(py, PY_POLL)?
                                .call1(py, (id as i64,))?
                                .extract::<WrapResult<_, _>>(py)?.0)
                            };
                            match res() {
                                Ok(res) => tx.send(res).or_log_ignore("Channel error (Module Manager)"),
                                Err(e) => tx.send(Err(e.to_string())).or_log_ignore("Channel error (Module Manager)"),
                            }
                        }
                    };
                    Ok(py.None())
                }).map_err(|e| ModError::LoadModule(e.to_string()))
                    .or_log("Python error (Module Manager)");
            }
        });
        Ok(tx)
    }

    /// Find function module by name
    fn find_function_module(&self, module_name: &str) -> Result<&UnboundedSender<FuncCommand>, ModError> {
        self.function_modules.get(module_name).ok_or_else(|| ModError::NoSuchModule)
    }

    /// Find driver module by name
    fn find_driver_module(&self, module_name: &str) -> Result<&UnboundedSender<DriverCommand>, ModError> {
        self.driver_modules.get(module_name).ok_or_else(|| ModError::NoSuchModule)
    }

    /// Receive command respond from channel
    async fn receive<T>(rx: Receiver<Result<T, String>>) -> Result<T, ModError>{
        match rx.await {
            Ok(res) => res.map_err(|e| ModError::Module(e.into())),
            Err(e) => Err(ModError::Channel(e.to_string())),
        }
    } 

    /// Load function from data. Calls load_data
    pub async fn load_function(&self, module_name: &str, data: Data) -> Result<u64, ModError> {
        let module = self.find_function_module(module_name)?;
        let (tx, rx) = oneshot::channel();
        module.send(FuncCommand::LoadData(data, tx)).map_err(|e| ModError::Channel(e.to_string()))?;
        ModuleManager::receive(rx).await
    }

    /// Trigger function event. Calls event
    pub async fn function_event(&self, module_name: &str, id: u64, state: u16) -> Result<(), ModError> {
        let module = self.find_function_module(module_name)?;
        let (tx, rx) = oneshot::channel();
        module.send(FuncCommand::Event(id, state, tx)).map_err(|e| ModError::Channel(e.to_string()))?;
        ModuleManager::receive(rx).await
    }

    /// Load driver from data. Calls load_data
    pub async fn load_driver(&self, module_name: &str, data: Data) -> Result<u64, ModError> {
        let module = self.find_driver_module(module_name)?;
        let (tx, rx) = oneshot::channel();
        module.send(DriverCommand::LoadData(data, tx)).map_err(|e| ModError::Channel(e.to_string()))?;
        ModuleManager::receive(rx).await
    }

    /// Get a drivers name. Calls name
    pub async fn driver_name(&self, module_name: &str, id: u64) -> Result<String, ModError> {
        let module = self.find_driver_module(module_name)?;
        let (tx, rx) = oneshot::channel();
        module.send(DriverCommand::Name(id, tx)).map_err(|e| ModError::Channel(e.to_string()))?;
        ModuleManager::receive(rx).await
    }

    /// Poll a driver. Calls poll
    pub async fn driver_poll(&self, module_name: &str, id: u64) -> Result<Vec<u16>, ModError> {
        let module = self.find_driver_module(module_name)?;
        let (tx, rx) = oneshot::channel();
        module.send(DriverCommand::Poll(id, tx)).map_err(|e| ModError::Channel(e.to_string()))?;
        ModuleManager::receive(rx).await
    }
}
use std::{ops::Range, collections::HashMap, fmt::Display, sync::Arc};

use configfs::async_trait;
use key_module::Data;
use serde::{Serialize, Deserialize};

use crate::modules::{ExternalDriver, ModuleManager};

use self::mcp23017::{InputType, MCP23017DriverBuilder};

/// MCP23017 driver
pub mod mcp23017;

#[derive(Debug)]
/// Driver error
pub struct DriverError {
    msg: String
}

impl DriverError {
    pub fn new(msg: String) -> DriverError {
        DriverError { msg }
    }
}

impl Display for DriverError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.msg)
    }
}



#[derive(Debug, Clone, Serialize, Deserialize)]
/// Driver Type, used to serialize driver configs
pub enum DriverType {
    /// Basic MCP23017 driver
    MCP23017{
        name: String,
        address: u16,
        bus: Option<u8>,
        inputs: Vec<InputType>
    },
    /// External driver
    External{ module: String, driver: Data }
}

impl DriverType {
    /// Build driver from type
    async fn build(self, module_manager: Arc<ModuleManager>) -> Result<Driver, DriverError> {
        match self {
            DriverType::MCP23017 { name, address, bus, inputs } => Ok(Box::new(MCP23017DriverBuilder::from_data(name, address, bus, inputs).await?)),
            DriverType::External { module, driver } => ExternalDriver::new(module, driver, module_manager).await,
        }
    }
}

#[async_trait]
/// Driver interface
pub trait DriverInterface {
    /// Get the drivers name, used to bind the driver states to a layout
    fn name(&self) -> &str;
    /// State iterator
    fn iter(&self) -> std::slice::Iter<u16>;
    /// Poll a state
    fn poll(&self, idx: usize) -> u16;
    /// Poll a range of states
    fn poll_range(&self, range: &Range<usize>) -> Option<&[u16]>;
    /// Tick the driver. Used to update the driver state.
    async fn tick(&mut self);
    /// Driver Type
    fn to_driver_type(&self) -> DriverType; 
}

/// Driver Object
pub type Driver = Box<dyn DriverInterface+ Send + Sync>;

/// Driver Manager
pub struct DriverManager {
    drivers: HashMap<String, Driver>,
}

impl DriverManager {
    /// New
    pub fn new(drivers: HashMap<String, Driver>) -> DriverManager {
        DriverManager { drivers }
    }

    /// Get a driver by name
    pub fn get(&self, name: &str) -> Option<&Driver> {
        self.drivers.get(name)
    }

    /// Tick drivers
    pub async fn tick(&mut self) {
        for driver in self.drivers.values_mut() {
            driver.tick().await;
        }
    }
}

/// Serializable Driver Manager
pub struct SerdeDriverManager {
    driver_manager: DriverManager,
    serde: Option<Vec<DriverType>>,
}

impl SerdeDriverManager {
    /// New
    pub fn new() -> SerdeDriverManager {
        SerdeDriverManager { driver_manager: DriverManager::new(HashMap::new()), serde: None }
    }

    #[allow(dead_code)]
    /// Create from a driver Manager
    pub fn load(driver_manager: DriverManager) -> SerdeDriverManager {
        SerdeDriverManager{driver_manager, serde: None}
    }

    /// Build a Driver Manager
    pub async fn build(self, module_manager: Arc<ModuleManager>) -> Result<DriverManager, DriverError> {
        if let Some(drivers) = self.serde {
            let mut driver_map = HashMap::new();

            for driver in drivers.into_iter() {
                let driver = driver.build(module_manager.clone()).await;
                let driver = driver.map_err(|e| DriverError::new(format!("{}", e)))?;

                if driver_map.insert(driver.name().to_string(), driver).is_some() {
                    return Err(DriverError::new("driver names must be unique".to_string()))
                }
            }
            Ok(DriverManager{drivers: driver_map})
        } else {
            Ok(self.driver_manager)
        }
    }
}

impl Serialize for SerdeDriverManager {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer 
    {
        self.driver_manager.drivers.values()
            .map(|d| d.to_driver_type())
            .collect::<Vec<DriverType>>()
            .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for SerdeDriverManager {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de> 
    {
        let drivers = Vec::<DriverType>::deserialize(deserializer)?;
        
        Ok(SerdeDriverManager{driver_manager: DriverManager{drivers: HashMap::new()}, serde: Some(drivers)})
    }
}
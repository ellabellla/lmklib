use std::{ops::Range, collections::HashMap, fmt::Display, sync::Arc, path::Path, fs, io::Write};

use async_trait::async_trait;
use itertools::Itertools;
use serde::{Serialize, Deserialize};

use crate::modules::{ExternalDriver, ModuleManager};

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
pub struct DriverData {
    pub module: String,
    pub data: String,
}

#[async_trait]
/// Driver interface
pub trait DriverInterface {
    /// State iterator
    fn iter(&self) -> std::slice::Iter<u16>;
    /// Poll a state
    fn poll(&self, idx: usize) -> u16;
    /// Poll a range of states
    fn poll_range(&self, range: &Range<usize>) -> Option<&[u16]>;
    /// Poll list of states
    fn poll_list(&self, idx: &Vec<usize>) -> Option<Vec<u16>>;
    /// Output a state
    async fn set(&mut self, idx: usize, state: u16);
    /// Tick the driver. Used to update the driver state.
    async fn tick(&mut self);
    /// Driver Type
    fn to_driver_data(&self) -> DriverData; 
}

/// Driver Object
pub type Driver = Box<dyn DriverInterface+ Send + Sync>;

/// Driver Manager
pub struct DriverManager {
    drivers: HashMap<String, Driver>,
}

impl DriverManager {
    #[allow(dead_code)]
    /// New
    pub fn new(drivers: HashMap<String, Driver>) -> DriverManager {
        DriverManager { drivers }
    }

    /// Load driver configurations from folder
    pub async fn load(drivers: &Path, module_manager: Arc<ModuleManager>) -> Result<DriverManager, DriverError> {
        let contents = fs::read_dir(drivers).map_err(|e| DriverError::new(format!("{}", e)))?;

        let mut drivers = HashMap::new();
        
        for entry in contents {
            let entry = entry.map_err(|e| DriverError::new(format!("{}", e)))?;
            let mut module_and_name = entry.path().file_stem()
                .ok_or_else(|| DriverError::new(format!("Unable to resolve file name, {:?}", entry.path())))?
                .to_string_lossy()
                .to_string()
                .split('-')
                .map(|s| s.to_owned())
                .take(2)
                .collect_vec();
            if module_and_name.len() != 2 {
                return Err(DriverError::new(format!("Unable to resolve file name, {:?}", entry.path())))
            }

            let [name, module] = [module_and_name.remove(0), module_and_name.remove(0)];

            let data = fs::read_to_string(entry.path()).map_err(|e| DriverError::new(format!("{}", e)))?;

            if drivers.contains_key(&name) {
                return Err(DriverError::new("Driver name already taken".to_string()))
            }

            let driver: Driver = ExternalDriver::new(module.to_string(), data, module_manager.clone()).await
                    .map_err(|e| DriverError::new(format!("{}", e)))?;

            drivers.insert(name, driver);
        }

        Ok(DriverManager { drivers })
    }

    #[allow(dead_code)]
    /// Serialize driver configuration to driver folder
    pub fn serialize(&self, drivers: &Path) -> Result<(), DriverError> {
        for (name, driver) in &self.drivers {
            match driver.to_driver_data() {
                DriverData{module, data} => {
                    fs::File::create(drivers.join(format!("{}-{}", name, module)))
                        .map_err(|e| DriverError::new(format!("{}", e)))?
                        .write_all(data.as_bytes())
                        .map_err(|e| DriverError::new(format!("{}", e)))?;
                }
            }
        }

        Ok(())
    }

    /// Get a driver by name
    pub fn get(&self, name: &str) -> Option<&Driver> {
        self.drivers.get(name)
    }

    #[allow(dead_code)]
    /// Get a driver by name
    pub fn get_mut(&mut self, name: &str) -> Option<&mut Driver> {
        self.drivers.get_mut(name)
    }

    /// Tick drivers
    pub async fn tick(&mut self) {
        for driver in self.drivers.values_mut() {
            driver.tick().await;
        }
    }
}
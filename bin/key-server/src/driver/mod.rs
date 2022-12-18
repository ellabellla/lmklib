use std::{ops::Range, collections::HashMap, fmt::Display};

use configfs::async_trait;
use serde::{Serialize, Deserialize};

use self::mcp23017::{InputType, MCP23017DriverBuilder};

pub mod mcp23017;

#[derive(Debug)]
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
pub enum DriverType {
    MCP23017{
        name: String,
        address: u16,
        bus: Option<u8>,
        inputs: Vec<InputType>
    }
}

impl DriverType {
    async fn build(self) -> Result<Driver, DriverError> {
        match self {
            DriverType::MCP23017 { name, address, bus, inputs } => Ok(Box::new(MCP23017DriverBuilder::from_data(name, address, bus, inputs).await?)),
        }
    }
}

#[async_trait]
pub trait DriverInterface {
    fn name(&self) -> &str;
    fn iter(&self) -> std::slice::Iter<u16>;
    fn poll(&self, idx: usize) -> u16;
    fn poll_range(&self, range: &Range<usize>) -> Option<&[u16]>;
    async fn tick(&mut self);
    fn to_driver_type(&self) -> DriverType; 
}

pub type Driver = Box<dyn DriverInterface+ Send + Sync>;

pub struct DriverManager {
    drivers: HashMap<String, Driver>,
}

impl DriverManager {
    pub fn new(drivers: HashMap<String, Driver>) -> DriverManager {
        DriverManager { drivers }
    }

    pub fn get(&self, name: &str) -> Option<&Driver> {
        self.drivers.get(name)
    }

    pub async fn tick(&mut self) {
        for driver in self.drivers.values_mut() {
            driver.tick().await;
        }
    }
}

pub struct SerdeDriverManager {
    driver_manager: DriverManager,
    serde: Option<Vec<DriverType>>,
}

impl SerdeDriverManager {
    pub fn new() -> SerdeDriverManager {
        SerdeDriverManager { driver_manager: DriverManager::new(HashMap::new()), serde: None }
    }

    #[allow(dead_code)]
    pub fn load(driver_manager: DriverManager) -> SerdeDriverManager {
        SerdeDriverManager{driver_manager, serde: None}
    }

    pub async fn build(self) -> Result<DriverManager, DriverError> {
        if let Some(drivers) = self.serde {
            let mut driver_map = HashMap::new();

            for driver in drivers.into_iter() {
                let driver = driver.build().await;
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
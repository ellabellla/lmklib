use std::{ops::Range, collections::HashMap};

use serde::{Serialize, Deserialize, de::{self}};

pub mod mcp23017;

#[typetag::serde]
pub trait DriverInterface {
    fn name(&self) -> &str;
    fn iter(&self) -> std::slice::Iter<u16>;
    fn poll(&self, idx: usize) -> u16;
    fn poll_range(&self, range: &Range<usize>) -> Option<&[u16]>;
    fn tick(&mut self);
}

pub type Driver = Box<dyn DriverInterface>;

pub struct DriverManager {
    pub(crate) drivers: HashMap<String, Driver>,
}

impl DriverManager {
    pub fn get(&self, name: &str) -> Option<&Driver> {
        self.drivers.get(name)
    }

    pub fn get_mut(&mut self, name: &str) -> Option<&mut Driver> {
        self.drivers.get_mut(name)
    }
}

impl Serialize for DriverManager {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer 
    {
        self.drivers.values().collect::<Vec<&Driver>>().serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for DriverManager {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de> 
    {
        let drivers = Vec::<Driver>::deserialize(deserializer)?;
        let mut driver_map = HashMap::new();
        for driver in drivers {
            if driver_map.insert(driver.name().to_string(), driver).is_some() {
                return Err(de::Error::custom("driver names must be unique"))
            }
        }
        Ok(DriverManager{drivers: driver_map})
    }
}
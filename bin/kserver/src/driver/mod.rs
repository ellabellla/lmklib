use std::{ops::Range, collections::HashMap};

use serde::{Serialize, ser::SerializeSeq, Deserialize, de::{Visitor, self}};

pub mod mcp23017;

#[typetag::serde]
pub trait DigitalDriver {
    fn name(&self) -> &str;
    fn iter(&self) -> std::slice::Iter<bool>;
    fn poll(&self, idx: usize) -> bool;
    fn poll_range(&self, range: &Range<usize>) -> Option<&[bool]>;
    fn tick(&mut self);
}

#[typetag::serde]
pub trait AnalogDriver {
    fn name(&self) -> &str;
    fn iter(&self) -> std::slice::Iter<u16>;
    fn poll(&self, idx: usize) -> u16;
    fn poll_range(&self, range: &Range<usize>) -> Option<&[u16]>;
    fn tick(&mut self);
}

pub enum Driver {
    Digital(Box<dyn DigitalDriver>),
    Analog(Box<dyn AnalogDriver>),
}

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
        let mut seq = serializer.serialize_seq(Some(self.drivers.len()))?;
        for (_, driver) in &self.drivers {
            match driver {
                Driver::Digital(driver) =>  seq.serialize_element(driver)?,
                Driver::Analog(driver) =>  seq.serialize_element(driver)?,
            };
        }
        seq.end()
    }
}

impl<'de> Deserialize<'de> for DriverManager {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de> 
    {
        deserializer.deserialize_seq(DriverManager{drivers: HashMap::new()})
    }
}

impl<'de> Visitor<'de> for DriverManager {
    type Value = DriverManager;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("expected drivers")
    }

    fn visit_seq<A>(mut self, mut seq: A) -> Result<Self::Value, A::Error>
        where
            A: de::SeqAccess<'de>, 
    {
        loop {
            if let Ok(driver) = seq.next_element::<Box<dyn DigitalDriver>>() {
                let Some(driver) = driver else {
                    break;
                };

                if self.drivers.insert(driver.name().to_string(), Driver::Digital(driver)).is_some() {
                    return Err(de::Error::custom("driver names must be unique"))
                }
            } else if let Ok(driver) = seq.next_element::<Box<dyn AnalogDriver>>() {
                let Some(driver) = driver else {
                    break;
                };

                if self.drivers.insert(driver.name().to_string(), Driver::Analog(driver)).is_some() {
                    return Err(de::Error::custom("driver names must be unique"))
                }
            } else {
                return Err(de::Error::custom("driver configuration couldn't be loaded"))
            }
        }

        Ok(self)
    }
}

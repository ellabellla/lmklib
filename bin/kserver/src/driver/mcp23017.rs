use std::{collections::HashSet, ops::Range};

use mcp23017_rpi_lib::{Pin, MCP23017, Mode, State};
use serde::{Serialize, ser::SerializeMap, Deserialize, de::{Visitor, self}};

use super::DigitalDriver;


#[typetag::serde]
trait MCP23017Input {
    fn setup(&self, mcp: &mut MCP23017) -> Result<(), mcp23017_rpi_lib::Error>;
    fn read(&self, mcp: &MCP23017) -> Result<Vec<bool>, mcp23017_rpi_lib::Error>;
    fn pins(&self) -> Vec<Pin>;
    fn len(&self) -> usize;
}

struct Matrix {
    x: Vec<Pin>,
    y: Vec<Pin>,
}

impl Matrix {
    pub fn new(x: Vec<Pin>, y: Vec<Pin>) -> Matrix {
        Matrix { x, y}
    }
}

impl Serialize for Matrix {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer {
        let mut map = serializer.serialize_map(Some(2))?;
        map.serialize_entry("x", &self.x.iter().map(|x| u8::from(x)).collect::<Vec<u8>>())?;
        map.serialize_entry("y", &self.y.iter().map(|x| u8::from(x)).collect::<Vec<u8>>())?;
        map.end()
    }
}

impl<'de> Deserialize<'de> for Matrix {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de> {
        deserializer.deserialize_map(Matrix{x: vec![], y: vec![]})
    }
}

impl<'de> Visitor<'de> for Matrix {
    type Value = Matrix;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("expected matrix")
    }

    fn visit_map<A>(mut self, mut map: A) -> Result<Self::Value, A::Error>
        where
            A: serde::de::MapAccess<'de>, 
    {
        while let Some(key) =  map.next_key::<String>()? {
            if key.to_lowercase() == "x" {
                if self.x.len() != 0 {
                    continue;
                }

                let pins = map.next_value::<Vec<u8>>()?;
                let pins = pins.iter()
                    .map(|pin| Pin::new(*pin));
                if pins.clone().any(|pin| pin.is_none()) {
                    return Err(de::Error::custom("expected a pin number between 0 and 15"))
                }

                self.x.extend(pins.filter_map(|p| p));
            } else if key.to_lowercase() == "y" {
                if self.y.len() != 0 {
                    continue;
                }

                let pins = map.next_value::<Vec<u8>>()?;
                let pins = pins.iter()
                    .map(|pin| Pin::new(*pin));
                if pins.clone().any(|pin| pin.is_none()) {
                    return Err(de::Error::custom("expected a pin number between 0 and 15"))
                }

                self.y.extend(pins.filter_map(|p| p))
            }
        }    

        if self.x.len() == 0 {
            return Err(de::Error::custom("atleast one x pin must be given"))
        }
        if self.y.len() == 0 {
            return Err(de::Error::custom("atleast one y pin must be given"))
        }

        Ok(self)
    }
}

#[typetag::serde]
impl MCP23017Input for Matrix {
    fn setup(&self, mcp: &mut MCP23017) -> Result<(), mcp23017_rpi_lib::Error>{
        for x in &self.x {
            mcp.pin_mode(x, Mode::Input)?;
        }
        for y in &self.y {
            mcp.pin_mode(y, Mode::Output)?;
            mcp.output(y, State::Low)?;
        }

        Ok(())
    }

    fn read(&self, mcp: &MCP23017) -> Result<Vec<bool>, mcp23017_rpi_lib::Error> {
        let mut out = Vec::with_capacity(self.x.len() * self.y.len());
        for y in &self.y {
            mcp.output(y, State::High)?;
            for x in &self.x {
                out.push(mcp.input(x)?.into());
            }
            mcp.output(y, State::Low)?;
        }

        Ok(out)
    }

    fn pins(&self) -> Vec<Pin> {
        let mut pins = Vec::with_capacity(self.x.len() + self.y.len());
        pins.extend(self.x.clone());
        pins.extend(self.y.clone());
        pins
    }

    fn len(&self) -> usize {
        self.x.len() * self.y.len()
    }
}

struct Input {
    pin: Pin,
    on_state: bool,
    pull_high: bool,
}

impl Input {
    pub fn new(pin: Pin, on_state: State, pull_high: bool) -> Input {
        Input { pin, on_state: on_state.into(), pull_high }
    }
}

impl Serialize for Input {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer {
        let mut map = serializer.serialize_map(Some(3))?;
        map.serialize_entry("pin", &u8::from(&self.pin))?;
        map.serialize_entry("on_state", &self.on_state)?;
        map.serialize_entry("pull_high", &self.pull_high)?;
        map.end()
    }
}

impl<'de> Deserialize<'de> for Input {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de> 
    {
        deserializer.deserialize_map(InputVisitor{pin: None, on_state: false, pull_high: false})    
    }
}

struct InputVisitor{
    pin: Option<Pin>,
    on_state: bool,
    pull_high: bool,
}

impl<'de> Visitor<'de> for InputVisitor {
    type Value = Input;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("expected input")
    }

    fn visit_map<A>(mut self, mut map: A) -> Result<Self::Value, A::Error>
        where
            A: serde::de::MapAccess<'de>, 
    {
        while let Some(key) =  map.next_key::<String>()? {
            if key.to_lowercase() == "pin" {
                if self.pin.is_some() {
                    continue;
                }
                let Some(pin) = Pin::new(map.next_value()?) else {
                    return Err(de::Error::custom("expected a pin number between 0 and 15"))
                };
                self.pin = Some(pin);
            } else if key.to_lowercase() == "on_state" {
               self.on_state = map.next_value()?;
            } else if key.to_lowercase() == "pull_high" {
                self.pull_high = map.next_value()?;
            }
        }    

        let Some(pin) = self.pin else {
            return Err(de::Error::custom("a pin number must be given"))
        };

        Ok(Input{ pin: pin, on_state: self.on_state, pull_high: self.pull_high })
    }
} 

#[typetag::serde]
impl MCP23017Input for Input {
    fn setup(&self, mcp: &mut MCP23017) -> Result<(), mcp23017_rpi_lib::Error> {
        mcp.pin_mode(&self.pin, Mode::Input)?;
        if self.pull_high {
            mcp.pull_up(&self.pin, State::High)?;
        }
        Ok(())
    }

    fn read(&self, mcp: &MCP23017) -> Result<Vec<bool>, mcp23017_rpi_lib::Error> {
        let state: bool = mcp.input(&self.pin)?.into();
        Ok(vec![state == self.on_state])
    }

    fn pins(&self) -> Vec<Pin> {
        vec![self.pin.clone()]
    }

    fn len(&self) -> usize {
        1
    }
}

pub struct MCP23017DriverBuilder {
    used: HashSet<Pin>,
    inputs: Vec<Box<dyn MCP23017Input>>,

    output_size: usize,
    name: String,
    address: u16,
    bus: u8,
}

impl MCP23017DriverBuilder {
    pub fn new(name: &str, address: u16, bus: u8) -> MCP23017DriverBuilder {
        MCP23017DriverBuilder { used: HashSet::new(), inputs: Vec::new(), output_size: 0, address, bus, name: name.to_string() }
    }

    pub fn add_matrix(&mut self, x: Vec<Pin>, y: Vec<Pin>) -> Option<Range<usize>> {
        for x in &x {
            if !self.used.insert(x.clone()) {
                return None;
            }
        }
        for y in &y {
            if !self.used.insert(y.clone()) {
                return None;
            }
        }

        let size = x.len() * y.len();
        let idx = self.inputs.len();
        self.output_size += size;
        self.inputs.push(Box::new(Matrix::new(x, y)));
        Some(Range {start: idx, end: idx+size})
    }

    pub fn add_input(&mut self, pin: Pin, on_state: State, pull_high: bool) -> Option<usize> {
        if !self.used.insert(pin.clone()) {
            return None;
        }

        let idx = self.inputs.len();
        self.output_size += 1;
        self.inputs.push(Box::new(Input::new(pin, on_state, pull_high)));
        Some(idx)
    }

    pub fn build<'a>(self) -> Result<MCP23017Driver, mcp23017_rpi_lib::Error> {
        let mut mcp = MCP23017::new(self.address, self.bus)?;
        mcp.reset()?;
        for input in self.inputs.iter() {
            input.setup(&mut mcp)?;
        }
        Ok(MCP23017Driver { 
            name: self.name,
            address: self.address,
            bus: self.bus,
            inputs: self.inputs, 
            state: Vec::with_capacity(self.output_size),
            mcp,
        })
    }
}


impl<'de> Deserialize<'de> for MCP23017DriverBuilder {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de> 
    {
        deserializer.deserialize_map(MCP23017DriverBuilder{
            used: HashSet::new(), 
            inputs: Vec::new(),  
            output_size: 0, 
            address: 0, 
            bus: 0, 
            name: "".to_string()
        })
    }
}


impl<'de> Visitor<'de> for MCP23017DriverBuilder {
    type Value = MCP23017DriverBuilder;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("expected mcp23017 driver")
    }

    fn visit_map<A>(mut self, mut map: A) -> Result<Self::Value, A::Error>
        where
            A: de::MapAccess<'de>, 
    {
        let mut address = None;
        let mut bus = None;
        let mut name = None;
        while let Some(key) =  map.next_key::<String>()? {
            if key.to_lowercase() == "inputs" {
                if self.inputs.len() != 0 {
                    continue;
                }
               
                self.inputs = map.next_value()?;
            } else if key.to_lowercase() == "address" {
                if address.is_some() {
                    continue;
                }

               address = Some(map.next_value()?);
            } else if key.to_lowercase() == "bus" {
                if bus.is_some() {
                    continue;
                }

               bus = Some(map.next_value()?);
            } else if key.to_lowercase() == "name" {
                if name.is_some() {
                    continue;
                }

               name = Some(map.next_value()?);
            }
        }

        let Some(address) = address else {
            return Err(de::Error::custom("an address must be supplied"))
        };
        let Some(name) = name else {
            return Err(de::Error::custom("a name must be supplied"))
        };

        self.name = name;
        self.address = address;
        self.bus = bus.unwrap_or(1);

        for input in &self.inputs {
            self.output_size += input.len();
            for pin in input.pins() {
                if !self.used.insert(pin.clone()) {
                    return Err(de::Error::custom(format!("pin {} cannot be reused", pin)));
                }
            }
        }

        Ok(self)
    }
}


pub struct MCP23017Driver {
    name: String,
    address: u16,
    bus: u8,
    inputs: Vec<Box<dyn MCP23017Input>>,
    state: Vec<bool>,
    mcp: MCP23017,
}

#[typetag::serde]
impl DigitalDriver for MCP23017Driver {
    fn name(&self) -> &str {
        &self.name
    }

    fn iter(&self) -> std::slice::Iter<bool> {
        self.state.iter()
    }

    fn poll(&self, idx: usize) -> bool {
        self.state.get(idx).map(|b| b.to_owned()).unwrap_or(false)
    }

    fn poll_range(&self, range: &Range<usize>) -> Option<&[bool]> {
        self.state.get(range.clone())
    }

    fn tick(&mut self) {
        let mut state: Vec<bool> = Vec::with_capacity(self.state.len());
        for input in &self.inputs {
            let Ok(input_state) = input.read(&self.mcp) else {
                return;
            };
            state.extend(input_state);

        }
        self.state = state;
    }
}

impl Serialize for MCP23017Driver {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer 
    {
        let mut map = serializer.serialize_map(Some(3))?;
        map.serialize_entry("name", &self.name)?;
        map.serialize_entry("address", &self.address)?;
        map.serialize_entry("bus", &self.bus)?;
        map.serialize_entry("inputs", &self.inputs)?;
        map.end()
    }
}

impl<'de> Deserialize<'de> for MCP23017Driver {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de> 
    {
        let builder = MCP23017DriverBuilder::deserialize(deserializer)?;

        builder.build().map_err(|e| de::Error::custom(format!("unable to build driver, {}", e)))
    }
}
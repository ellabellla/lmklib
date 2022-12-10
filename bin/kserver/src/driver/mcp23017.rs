use std::{collections::HashSet, ops::Range};

use mcp23017_rpi_lib::{Pin, MCP23017, Mode, State};
use serde::{Serialize, Deserialize, de::{self}};

use super::{DriverInterface};


#[typetag::serde]
trait MCP23017Input {
    fn setup(&self, mcp: &mut MCP23017) -> Result<(), mcp23017_rpi_lib::Error>;
    fn read(&self, mcp: &MCP23017) -> Result<Vec<u16>, mcp23017_rpi_lib::Error>;
    fn pins(&self) -> Vec<Pin>;
    fn len(&self) -> usize;
}

fn bool_int(bool: bool) -> u16 {
    if bool {
        1
    } else {
        0
    }
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
        S: serde::Serializer 
    {
        #[derive(Serialize)]
        struct Matrix {
            x: Vec<u8>,
            y: Vec<u8>
        }

        Matrix{
            x: self.x.iter().map(|x| u8::from(x)).collect::<Vec<u8>>(),
            y: self.y.iter().map(|x| u8::from(x)).collect::<Vec<u8>>()
        }.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Matrix {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de> 
    {
        #[derive(Deserialize)]
        struct Matrix {
            x: Vec<u8>,
            y: Vec<u8>
        }

        let matrix = Matrix::deserialize(deserializer)?;

        if matrix.x.len() == 0 {
            return Err(de::Error::custom("atleast one x pin must be given"))
        }
        if matrix.y.len() == 0 {
            return Err(de::Error::custom("atleast one y pin must be given"))
        }

        let x = matrix.x.iter()
            .map(|pin| Pin::new(*pin));
        if x.clone().any(|pin| pin.is_none()) {
            return Err(de::Error::custom("expected a pin number between 0 and 15"))
        }

        let y = matrix.y.iter()
            .map(|pin| Pin::new(*pin));
        if y.clone().any(|pin| pin.is_none()) {
            return Err(de::Error::custom("expected a pin number between 0 and 15"))
        }

        Ok(super::mcp23017::Matrix{
            x: x.filter_map(|p| p).collect(),
            y: y.filter_map(|p| p).collect(),
        })
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

    fn read(&self, mcp: &MCP23017) -> Result<Vec<u16>, mcp23017_rpi_lib::Error> {
        let mut out = Vec::with_capacity(self.x.len() * self.y.len());
        for y in &self.y {
            mcp.output(y, State::High)?;
            for x in &self.x {
                out.push(bool_int(mcp.input(x)?.into()));
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
        S: serde::Serializer 
    {
        #[derive(Serialize)]
        struct Input {
            pin: u8,
            on_stage: bool,
            pull_high: bool,
        }
        Input{pin: u8::from(&self.pin), on_stage: self.on_state, pull_high: self.pull_high}
        .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Input {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de> 
    {
        #[derive(Deserialize)]
        struct Input {
            pin: u8,
            on_stage: bool,
            pull_high: bool,
        }
        let input = Input::deserialize(deserializer)?;
        let Some(pin) = Pin::new(input.pin) else {
            return Err(de::Error::custom("expected a pin number between 0 and 15"))
        };
        Ok(super::mcp23017::Input{
            pin,
            on_state: input.on_stage,
            pull_high: input.pull_high
        })
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

    fn read(&self, mcp: &MCP23017) -> Result<Vec<u16>, mcp23017_rpi_lib::Error> {
        let state: bool = mcp.input(&self.pin)?.into();
        Ok(vec![bool_int(state == self.on_state)])
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
        #[derive(Deserialize)]
        struct MCP23017 {
            name: String,
            address: u16,
            bus: Option<u8>,
            inputs: Vec<Box<dyn MCP23017Input>>
        };

        let mcp = MCP23017::deserialize(deserializer)?;
        let mut used = HashSet::new();
        let mut output_size = 0;
        for input in &mcp.inputs {
            output_size += input.len();
            for pin in input.pins() {
                if !used.insert(pin.clone()) {
                    return Err(de::Error::custom(format!("pin {} cannot be reused", pin)));
                }
            }
        }

        Ok(MCP23017DriverBuilder{
            used, 
            inputs: mcp.inputs,  
            output_size, 
            address: mcp.address, 
            bus: mcp.bus.unwrap_or(1), 
            name: mcp.name
        })
    }
}


pub struct MCP23017Driver {
    name: String,
    address: u16,
    bus: u8,
    inputs: Vec<Box<dyn MCP23017Input>>,
    state: Vec<u16>,
    mcp: MCP23017,
}

#[typetag::serde]
impl DriverInterface for MCP23017Driver {
    fn name(&self) -> &str {
        &self.name
    }

    fn iter(&self) -> std::slice::Iter<u16> {
        self.state.iter()
    }

    fn poll(&self, idx: usize) -> u16 {
        self.state.get(idx).map(|b| b.to_owned()).unwrap_or(0)
    }

    fn poll_range(&self, range: &Range<usize>) -> Option<&[u16]> {
        self.state.get(range.clone())
    }

    fn tick(&mut self) {
        let mut state: Vec<u16> = Vec::with_capacity(self.state.len());
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
        #[derive(Serialize)]
        struct MCP23017<'a> {
            name: &'a str,
            address: u16,
            bus: u8,
            inputs: &'a Vec<Box<dyn MCP23017Input>>
        }
        MCP23017{name: &self.name, address: self.address, bus: self.bus, inputs: &self.inputs}
        .serialize(serializer)
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
use std::{collections::{HashSet}, ops::Range, fmt::Display};

use abi_stable::{std_types::{RVec, RString, RResult::{RErr, ROk, self}}, traits::IntoReprC, export_root_module, sabi_extern_fn, prefix_type::PrefixTypeTrait, sabi_trait::TD_Opaque};
use key_module::driver::{Driver, DriverModuleRef, DriverBox, DriverModule};
use mcp23017_rpi_lib::{Pin, Mode, State, MCP23017};
use serde::{Serialize, Deserialize};


#[export_root_module]
pub fn get_library() -> DriverModuleRef {
    DriverModule {
        new_driver    
    }
    .leak_into_prefix()
}

#[sabi_extern_fn]
fn new_driver() -> DriverBox {
    DriverBox::from_value(MCPModule{drivers: Vec::new()}, TD_Opaque)
}


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
/// Pin type, used to serialize pin configurations
pub enum PinType {
    /// Matrix pin configuration
    Matrix {
        x: Vec<u8>,
        y: Vec<u8>
    },
    /// Single pin input
    Input {
        pin: u8,
        on_state: bool,
        pull_high: bool,
    },
    /// Single pin output
    Output {
        pin: u8,
    }
}

impl PinType {
    /// Build an input
    fn build(self) -> Result<PinConfig, DriverError> {
        match self {
            PinType::Matrix { x, y } => Ok(Box::new(Matrix::new(x, y)?)),
            PinType::Input { pin, on_state, pull_high } => Ok(Box::new(Input::new(pin, on_state, pull_high)?)),
            PinType::Output { pin } => Ok(Box::new(Output::new(pin)?)),
        }
    }
}


/// Pin Configuration Object
type PinConfig = Box<dyn PinConfiguration + Send + Sync>;

/// Pin Configuration interface
trait PinConfiguration {
    /// Setup
    fn setup(&self, mcp: &mut MCP23017) -> Result<(), DriverError>;
    /// Read inputs
    fn read(&self, mcp: &MCP23017) -> Result<Vec<u16>, DriverError>;
    /// Set output
    fn set(&mut self, mcp: &MCP23017, idx: usize, state: u16) -> Result<(), DriverError>;
    /// List of pins
    fn pins(&self) -> Vec<Pin>;
    /// Number of pins
    fn len(&self) -> usize;
    /// Input Type
    fn to_pin_type(&self) -> PinType; 
}

/// Convert bool to int
fn bool_int(bool: bool) -> u16 {
    if bool {
        1
    } else {
        0
    }
}

/// Matrix input
/// 
/// Creates a matrix of pins where the x pins are inputs and the y pins are outputs.
/// Each y pin is set high then the x pins are scanned and, high inputs are taken 
/// as on and low are off for that point in the matrix.
struct Matrix {
    x: Vec<Pin>,
    y: Vec<Pin>,
}

impl Matrix {
    /// New
    pub fn new(x: Vec<u8>, y: Vec<u8>) -> Result<Matrix, DriverError> {
        let x = x.iter()
            .map(|pin| Pin::new(*pin));
        if x.clone().any(|pin| pin.is_none()) {
            return Err(DriverError::new("expected a pin number between 0 and 15".to_string()))
        }

        let y = y.iter()
            .map(|pin| Pin::new(*pin));
        if y.clone().any(|pin| pin.is_none()) {
            return Err(DriverError::new("expected a pin number between 0 and 15".to_string()))
        }

        Ok(Matrix{
            x: x.filter_map(|p| p).collect(),
            y: y.filter_map(|p| p).collect(),
        })
    }
}

impl PinConfiguration for Matrix {
    fn setup(&self, mcp: &mut MCP23017) -> Result<(), DriverError>{
        for x in &self.x {
            mcp.pin_mode(x, Mode::Input).map_err(|e| DriverError::new(format!("{}", e)))?;
        }
        for y in &self.y {
            mcp.pin_mode(y, Mode::Output).map_err(|e| DriverError::new(format!("{}", e)))?;
            mcp.output(y, State::Low).map_err(|e| DriverError::new(format!("{}", e)))?;
        }

        Ok(())
    }

    fn read(&self, mcp: &MCP23017) -> Result<Vec<u16>, DriverError> {
        let mut out = Vec::with_capacity(self.x.len() * self.y.len());
        for y in &self.y {
            mcp.output(y, State::High).map_err(|e| DriverError::new(format!("{}", e)))?;
            for x in &self.x {
                out.push(bool_int(mcp.input(x).map_err(|e| DriverError::new(format!("{}", e)))?.into()));
            }
            mcp.output(y, State::Low).map_err(|e| DriverError::new(format!("{}", e)))?;
        }

        Ok(out)
    }

    fn set(&mut self, _mcp: &MCP23017, _idx: usize, _state: u16) -> Result<(), DriverError> {
        Err(DriverError::new("Input is not settable".to_string()))
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

    fn to_pin_type(&self) -> PinType {
        PinType::Matrix{
            x: self.x.iter().map(|x| u8::from(x)).collect::<Vec<u8>>(),
            y: self.y.iter().map(|x| u8::from(x)).collect::<Vec<u8>>()
        }
    } 
}

/// Input 
/// 
/// A single pin is set to input and polled for it's state.
struct Input {
    pin: Pin,
    on_state: bool,
    pull_high: bool,
}

impl Input {
    /// New, on_state will invert High to Low when true, pull high will pull the input high when true.
    pub fn new(pin: u8, on_state: bool, pull_high: bool) -> Result<Input, DriverError> {
        let pin = Pin::new(pin).ok_or_else(|| DriverError::new("expected a pin number between 0 and 15".to_string()))?;
        Ok(Input { pin, on_state, pull_high })
    }
}

impl PinConfiguration for Input {
    fn setup(&self, mcp: &mut MCP23017) -> Result<(), DriverError> {
        mcp.pin_mode(&self.pin, Mode::Input).map_err(|e| DriverError::new(format!("{}", e)))?;
        if self.pull_high {
            mcp.pull_up(&self.pin, State::High).map_err(|e| DriverError::new(format!("{}", e)))?;
        }
        Ok(())
    }

    fn read(&self, mcp: &MCP23017) -> Result<Vec<u16>, DriverError> {
        let state: bool = mcp.input(&self.pin).map_err(|e| DriverError::new(format!("{}", e)))?.into();
        Ok(vec![bool_int(state == self.on_state)])
    }

    fn set(&mut self, _mcp: &MCP23017, _idx: usize, _state: u16) -> Result<(), DriverError> {
        Err(DriverError::new("Input is not settable".to_string()))
    }

    fn pins(&self) -> Vec<Pin> {
        vec![self.pin.clone()]
    }

    fn len(&self) -> usize {
        1
    }

    fn to_pin_type(&self) -> PinType {
        PinType::Input{pin: u8::from(&self.pin), on_state: self.on_state, pull_high: self.pull_high}
    }
}

struct Output {
    pin: Pin,
    state: bool,
}

impl Output {
    pub fn new(pin: u8) -> Result<Output, DriverError> {
        let pin = Pin::new(pin).ok_or_else(|| DriverError::new("expected a pin number between 0 and 15".to_string()))?;
        Ok(Output { pin, state: false })
    }
}

impl PinConfiguration for Output {
    fn setup(&self, mcp: &mut MCP23017) -> Result<(), DriverError> {
        mcp.pin_mode(&self.pin, Mode::Output).map_err(|e| DriverError::new(format!("{}", e)))?;
        mcp.output(&self.pin, State::from(self.state)).map_err(|e| DriverError::new(format!("{}", e)))?;
        Ok(())
    }

    fn read(&self, _mcp: &MCP23017) -> Result<Vec<u16>, DriverError> {
        Ok(vec![bool_int(self.state)])
    }

    fn set(&mut self, mcp: &MCP23017, _idx: usize, state: u16) -> Result<(), DriverError> {
        self.state = state != 0;
        mcp.output(&self.pin, State::from(self.state)).map(|_| ())
            .map_err(|e| DriverError::new(format!("{}", e)))
    }

    fn pins(&self) -> Vec<Pin> {
        vec![self.pin.clone()]
    }

    fn len(&self) -> usize {
        1
    }

    fn to_pin_type(&self) -> PinType {
        PinType::Output { pin: u8::from(&self.pin) }
    }
}


#[derive(Serialize, Deserialize)]
/// MCP23017 Driver Data
pub struct MCP23017Data { 
    address: u16, 
    bus: Option<u8>, 
    inputs: Vec<PinType>
}

pub struct MCPModule {
    drivers: Vec<MCP23017Driver>,
}

impl Driver for MCPModule {
     /// Initialize new driver from key server config data
    /// Returns the id of the new driver
    fn load_data<'borr>(&mut self, data: RString) -> abi_stable::std_types::RResult<u64,RString> {
        let driver = match MCP23017Driver::from_data(data.to_string().as_ref())
        .map_err(|e| format!("{}", e).into()) {
            Ok(driver) => driver,
            Err(e) => return RErr(e)
        };

        self.drivers.push(driver);
        ROk((self.drivers.len() - 1) as u64)
    }

    /// Poll the current state of the driver with the specified id
    fn poll(&mut self, id: u64) -> RResult<RVec<u16>, RString> {
        if id >= self.drivers.len() as u64 {
            return RErr("Invalid driver id".to_owned().into_c())
        }

        ROk(self.drivers[id as usize].poll())
    }

    //. Set the current state of the driver with the specified id
    fn set(&mut self, id: u64, idx: usize, state: u16) -> RResult<(), RString> {
        if id >= self.drivers.len() as u64 {
            return RErr("Invalid driver id".to_owned().into_c())
        }

        self.drivers[id as usize].set(idx, state)
    }
}

/// MCP23017 Driver
pub struct MCP23017Driver {
    pins: Vec<PinConfig>,
    mcp: MCP23017,
    output_size: usize,
    pin_map: Vec<Range<usize>>,
}

impl MCP23017Driver {
    /// Load driver settings from data
    pub fn from_data(data: &str) -> Result<MCP23017Driver, DriverError> {
        let data: MCP23017Data = serde_json::from_str(&data).map_err(|e| DriverError::new(format!("Unable to parse MCP23017 data, {}", e)))?;

        let bus = data.bus.unwrap_or(1);
        let mut output_size = 0;
        let inputs = data.inputs.into_iter().map(|input| input.build());
        if let Some(res) = inputs.clone().find(|i | i.is_err()) {
            res?;
        }

        let mut mcp = MCP23017::new(data.address, bus).map_err(|e| DriverError::new(format!("{}", e)))?;
        mcp.reset().map_err(|e| DriverError::new(format!("{}", e)))?;
        
        let mut used = HashSet::new();
        let mut pins = Vec::with_capacity(inputs.len());
        for input in inputs {
            let pin = input.map_err(|e| DriverError::new(format!("Unable to parse MCP23017 data, {}", e)))?;
            for pin in pin.pins() {
                if !used.insert(pin.clone()) {
                    return Err(DriverError::new(format!("Pin {} cannot be reused", pin)))
                }
            }
            pin.setup(&mut mcp)?;
            output_size += pin.len();
            pins.push(pin)
        }

        Ok(MCP23017Driver { 
            pins, 
            mcp: mcp,
            pin_map: Vec::new(),
            output_size,
        })
    }

    fn poll(&mut self) -> RVec<u16> {
        let mut state: RVec<u16> = RVec::with_capacity(self.output_size);
        let mut map: Vec<Range<usize>> = Vec::with_capacity(self.pins.len());

        for input in &self.pins {
            let mcp = &self.mcp;
            let input_state = input.read(mcp).unwrap_or_else(|_| vec![0; input.len()]);
            let range = Range{start: state.len(), end: state.len() + input_state.len()};
            state.extend(input_state.into_iter());
            map.push(range);
        }
        self.pin_map = map;

        return state;
    }

    fn set(&mut self, idx: usize, state: u16) -> RResult<(), RString> {
        let mut count = 0;
        for (i, range) in self.pin_map.iter().enumerate() {
            if range.contains(&idx) {
                let mcp = &self.mcp;
                return match self.pins[i].set(mcp, idx - count, state) {
                    Ok(_) => ROk(()),
                    Err(e) => RErr(e.msg.into())
                }
            }
            count += range.len();
        }

        RErr("Unable to find pin".into())
    }
}
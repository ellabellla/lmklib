use std::{collections::{HashSet}, ops::Range};

use configfs::async_trait;
use itertools::Itertools;
use mcp23017_rpi_lib::{Pin, Mode, State};
use serde::{Serialize, Deserialize};
use tokio::{sync::{mpsc::{self, UnboundedSender}, oneshot}};

use super::{DriverInterface, DriverType, DriverError};
use crate::{OrLogIgnore, OrLog};

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

#[async_trait]
/// Pin Configuration interface
trait PinConfiguration {
    /// Setup
    async fn setup(&self, mcp: &mut MCP23017) -> Result<(), DriverError>;
    /// Read inputs
    async fn read(&self, mcp: &MCP23017) -> Result<Vec<u16>, DriverError>;
    /// Set output
    async fn set(&mut self, mcp: &MCP23017, idx: usize, state: u16) -> Result<(), DriverError>;
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

#[async_trait]
impl PinConfiguration for Matrix {
    async fn setup(&self, mcp: &mut MCP23017) -> Result<(), DriverError>{
        for x in &self.x {
            mcp.pin_mode(x, Mode::Input).await?;
        }
        for y in &self.y {
            mcp.pin_mode(y, Mode::Output).await?;
            mcp.output(y, State::Low).await?;
        }

        Ok(())
    }

    async fn read(&self, mcp: &MCP23017) -> Result<Vec<u16>, DriverError> {
        let mut out = Vec::with_capacity(self.x.len() * self.y.len());
        for y in &self.y {
            mcp.output(y, State::High).await?;
            for x in &self.x {
                out.push(bool_int(mcp.input(x).await?.into()));
            }
            mcp.output(y, State::Low).await?;
        }

        Ok(out)
    }

    async fn set(&mut self, _mcp: &MCP23017, _idx: usize, _state: u16) -> Result<(), DriverError> {
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

#[async_trait]
impl PinConfiguration for Input {
    async fn setup(&self, mcp: &mut MCP23017) -> Result<(), DriverError> {
        mcp.pin_mode(&self.pin, Mode::Input).await?;
        if self.pull_high {
            mcp.pull_up(&self.pin, State::High).await?;
        }
        Ok(())
    }

    async fn read(&self, mcp: &MCP23017) -> Result<Vec<u16>, DriverError> {
        let state: bool = mcp.input(&self.pin).await?.into();
        Ok(vec![bool_int(state == self.on_state)])
    }

    async fn set(&mut self, _mcp: &MCP23017, _idx: usize, _state: u16) -> Result<(), DriverError> {
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

#[async_trait]
impl PinConfiguration for Output {
    async fn setup(&self, mcp: &mut MCP23017) -> Result<(), DriverError> {
        mcp.pin_mode(&self.pin, Mode::Output).await?;
        mcp.output(&self.pin, State::from(self.state)).await?;
        Ok(())
    }

    async fn read(&self, _mcp: &MCP23017) -> Result<Vec<u16>, DriverError> {
        Ok(vec![bool_int(self.state)])
    }

    async fn set(&mut self, mcp: &MCP23017, _idx: usize, state: u16) -> Result<(), DriverError> {
        self.state = state != 0;
        mcp.output(&self.pin, State::from(self.state)).await.map(|_| ())
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



#[allow(dead_code)]
/// MCP23017 driver builder
pub struct MCP23017DriverBuilder {
    used: HashSet<Pin>,
    pins: Vec<PinConfig>,

    output_size: usize,
    name: String,
    address: u16,
    bus: u8,
}

impl MCP23017DriverBuilder {
    #[allow(dead_code)]
    /// New, drives chip at the address at the given bus.
    pub fn new(name: &str, address: u16, bus: u8) -> MCP23017DriverBuilder {
        MCP23017DriverBuilder { used: HashSet::new(), pins: Vec::new(), output_size: 0, address, bus, name: name.to_string() }
    }

    #[allow(dead_code)]
    /// Add matrix input
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
        let idx = self.pins.len();
        self.output_size += size;
        self.pins.push(Box::new(Matrix{x, y}));
        Some(Range {start: idx, end: idx+size})
    }

    #[allow(dead_code)]
    /// Add input
    pub fn add_input(&mut self, pin: Pin, on_state: State, pull_high: bool) -> Option<usize> {
        if !self.used.insert(pin.clone()) {
            return None;
        }

        let idx = self.pins.len();
        self.output_size += 1;
        self.pins.push(Box::new(Input{pin, on_state: on_state.into(), pull_high}));
        Some(idx)
    }

    #[allow(dead_code)]
    /// Add input
    pub fn add_output(&mut self, pin: Pin) -> Option<usize> {
        if !self.used.insert(pin.clone()) {
            return None;
        }

        let idx = self.pins.len();
        self.output_size += 1;
        self.pins.push(Box::new(Output{pin, state: false}));
        Some(idx)
    }

    #[allow(dead_code)]
    /// Build the driver
    pub async fn build<'a>(self) -> Result<MCP23017Driver, DriverError> {
        let mut mcp = MCP23017::new(self.address, self.bus).await?;
        mcp.reset().await?;
        for input in self.pins.iter() {
            input.setup(&mut mcp).await?;
        }
        Ok(MCP23017Driver { 
            name: self.name,
            address: self.address,
            bus: self.bus,
            pins: self.pins, 
            state: Vec::with_capacity(self.output_size),
            mcp: mcp,
            pin_map: Vec::new()
        })
    }

    /// Load driver settings from data
    pub async fn from_data(name: String, address: u16, bus: Option<u8>, pins: Vec<PinType>) -> Result<MCP23017Driver, DriverError> {
        let bus = bus.unwrap_or(1);
        let mut output_size = 0;
        let inputs = pins.into_iter().map(|input| input.build());
        if let Some(res) = inputs.clone().find(|i | i.is_err()) {
            res?;
        }

        let pins = inputs.filter_map(|i|i.or_log("Input build error (MCP23017)")).collect_vec();
        let mut mcp = MCP23017::new(address, bus).await?;
        mcp.reset().await?;
        
        let mut used = HashSet::new();
        for pin in pins.iter() {
            for pin in pin.pins() {
                if !used.insert(pin.clone()) {
                    return Err(DriverError::new(format!("Pin {} cannot be reused", pin)))
                }
            }
            pin.setup(&mut mcp).await?;
            output_size += pin.len();
        }

        Ok(MCP23017Driver { 
            name: name,
            address: address,
            bus: bus,
            pins, 
            state: Vec::with_capacity(output_size),
            mcp: mcp,
            pin_map: Vec::new()
        })
    }
}

/// MCP23017 Driver
pub struct MCP23017Driver {
    name: String,
    address: u16,
    bus: u8,
    pins: Vec<PinConfig>,
    state: Vec<u16>,
    mcp: MCP23017,
    pin_map: Vec<Range<usize>>,
}

#[async_trait]
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

    async fn set(&mut self, idx: usize, state: u16) {
        let mut count = 0;
        for (i, range) in self.pin_map.iter().enumerate() {
            if range.contains(&idx) {
                let mcp = &self.mcp;
                self.pins[i].set(mcp, idx - count, state).await.or_log("Pin set error (MCP23017 Driver)");
                return;
            }
            count += range.len();
        }
    }

    async fn tick(&mut self) {
        let mut state: Vec<u16> = Vec::with_capacity(self.state.len());
        let mut map: Vec<Range<usize>> = Vec::with_capacity(self.pins.len());

        for input in &self.pins {
            let mcp = &self.mcp;
            let Ok(input_state) = input.read(mcp).await else {
                return;
            };
            let range = Range{start: state.len(), end: state.len() + input_state.len()};
            state.extend(input_state.into_iter());
            map.push(range);
        }
        self.state = state;
        self.pin_map = map;
    }

    fn to_driver_type(&self) -> DriverType {
        DriverType::MCP23017 { 
            name: self.name.to_string(), 
            address: self.address, 
            bus: Some(self.bus), 
            inputs: self.pins.iter().map(|driver| driver.to_pin_type()).collect()
        }
    }
}

#[allow(dead_code)]
/// MCP23017 Command, used to send messages to the wrapped i2c controller
enum  MCP23017Command {
    PullUp(Pin, State, oneshot::Sender<Result<u16, mcp23017_rpi_lib::Error>>),
    PinMode(Pin, Mode, oneshot::Sender<Result<u16, mcp23017_rpi_lib::Error>>),
    Output(Pin, State, oneshot::Sender<Result<u8, mcp23017_rpi_lib::Error>>),
    Input(Pin, oneshot::Sender<Result<State, mcp23017_rpi_lib::Error>>),
    CurrentVal(Pin, oneshot::Sender<Result<State, mcp23017_rpi_lib::Error>>),
    ConfigSysInt(mcp23017_rpi_lib::Feature, State, oneshot::Sender<Result<(), mcp23017_rpi_lib::Error>>),
    ConfigPinInt(Pin, mcp23017_rpi_lib::Feature, mcp23017_rpi_lib::Compare, Option<State>, oneshot::Sender<Result<(), mcp23017_rpi_lib::Error>>),
    ReadInt(mcp23017_rpi_lib::Bank, oneshot::Sender<Result<Option<(Pin, State)>, mcp23017_rpi_lib::Error>>),
    ClearInt(oneshot::Sender<Result<(), mcp23017_rpi_lib::Error>>),
    Reset(oneshot::Sender<Result<(), mcp23017_rpi_lib::Error>>),
}

/// Wrapped MCP23017 i2c connection 
struct MCP23017 {
    tx: UnboundedSender<MCP23017Command>,
}

impl MCP23017 {
    /// New
    pub async fn new(address: u16, bus: u8) -> Result<MCP23017, DriverError> {
        let (tx, rx) = mpsc::unbounded_channel();
        let (new_tx, new_rx) = oneshot::channel();

        tokio::task::spawn_blocking(move || {
            let mut rx = rx;
            let mut mcp = match mcp23017_rpi_lib::MCP23017::new(address, bus)  {
                Ok(mcp) => {new_tx.send(Ok(())).or_log_ignore("Broken Channel (MCP23017 Driver)"); mcp},
                Err(e) => {new_tx.send(Err(DriverError::new(format!("MCP23017 Error, {}", e)))).or_log_ignore("Broken Channel (MCP23017 Driver)"); return},
            };

            while let Some(command) = rx.blocking_recv() {
                match command {
                    MCP23017Command::PullUp(pin, value, tx) => {tx.send(mcp.pull_up(&pin, value)).or_log_ignore("Broken Channel (MCP23017 Driver)");},
                    MCP23017Command::PinMode(pin, mode, tx) => {tx.send(mcp.pin_mode(&pin, mode)).or_log_ignore("Broken Channel (MCP23017 Driver)");},
                    MCP23017Command::Output(pin, value, tx) => {tx.send(mcp.output(&pin, value)).or_log_ignore("Broken Channel (MCP23017 Driver)");},
                    MCP23017Command::Input(pin, tx) => {tx.send(mcp.input(&pin)).or_log_ignore("Broken Channel (MCP23017 Driver)");},
                    MCP23017Command::CurrentVal(pin, tx) => {tx.send(mcp.current_val(&pin)).or_log_ignore("Broken Channel (MCP23017 Driver)");},
                    MCP23017Command::ConfigSysInt(mirror, intpol, tx) => {tx.send(mcp.config_system_interrupt(mirror, intpol)).or_log_ignore("Broken Channel (MCP23017 Driver)");},
                    MCP23017Command::ConfigPinInt(pin, enabled, compare_mode, defval, tx) => {tx.send(mcp.config_pin_interrupt(&pin, enabled, compare_mode, defval)).or_log_ignore("Broken Channel (MCP23017 Driver)");},
                    MCP23017Command::ReadInt(port, tx) => {tx.send(mcp.read_interrupt(port)).or_log_ignore("Broken Channel (MCP23017 Driver)");},
                    MCP23017Command::ClearInt(tx) => {tx.send(mcp.clear_interrupts()).or_log_ignore("Broken Channel (MCP23017 Driver)");},
                    MCP23017Command::Reset(tx) => {tx.send(mcp.reset()).or_log_ignore("Broken Channel (MCP23017 Driver)");},
                };
            }
        });

        let Ok(res) = new_rx.await else {
            return Err(DriverError::new("Unable to create MCP23017 Driver".to_string()))
        };
        res.map(|_| MCP23017 { tx })
    }

    /// Send command
    async fn send(&self, command: MCP23017Command) -> Result<(), DriverError> {
        self.tx.send(command).map_err(|_|DriverError::new("Unable to call MCP23017".to_string()))
    }

    /// Receive command
    async fn receive<T>(&self, rx: oneshot::Receiver<Result<T, mcp23017_rpi_lib::Error>>) -> Result<T, DriverError> {
        if let Ok(val) = rx.await {
            val.map_err(|e| DriverError::new(format!("MCP23017 Error, {}", e)))
        } else {
            return Err(DriverError::new("Unable to call MCP23017".to_string()));
        }
    }

    /// Pull pin high
    pub  async fn pull_up(&self, pin: &Pin, value: State) -> Result<u16, DriverError> {
        let (tx, rx) = oneshot::channel();
        let command = MCP23017Command::PullUp(pin.clone(), value, tx);
        self.send(command).await?;
        self.receive(rx).await
    }

    /// Set pin mode
    pub async fn pin_mode(&mut self, pin: &Pin, mode: Mode) -> Result<u16, DriverError> {
        let (tx, rx) = oneshot::channel();
        let command = MCP23017Command::PinMode(pin.clone(), mode, tx);
        
        self.send(command).await?;
        self.receive(rx).await
    }

    /// Set pin output
    pub async fn output(&self, pin: &Pin, value: State) -> Result<u8, DriverError>{
        let (tx, rx) = oneshot::channel();
        let command = MCP23017Command::Output(pin.clone(), value, tx);
        
        self.send(command).await?;
        self.receive(rx).await
    }

    /// Get pin input
    pub async fn input(&self, pin: &Pin) -> Result<State, DriverError> {
        let (tx, rx) = oneshot::channel();
        let command = MCP23017Command::Input(pin.clone(), tx);
        
        self.send(command).await?;
        self.receive(rx).await
    }


    #[allow(dead_code)]
    /// Get the current value of a pin
    pub async fn current_val(&self, pin: &Pin) -> Result<State, DriverError> {
        let (tx, rx) = oneshot::channel();
        let command = MCP23017Command::CurrentVal(pin.clone(), tx);
        
        self.send(command).await?;
        self.receive(rx).await
    }

    #[allow(dead_code)]
    /// Configure system interrupt settings.
    /// Mirror - are the int pins mirrored?
    /// Intpol - polarity of the int pin.
    pub async fn config_system_interrupt(&mut self, mirror: mcp23017_rpi_lib::Feature, intpol: State) -> Result<(), DriverError>{
        let (tx, rx) = oneshot::channel();
        let command = MCP23017Command::ConfigSysInt(mirror, intpol, tx);
        
        self.send(command).await?;
        self.receive(rx).await
    }

    #[allow(dead_code)]
    /// Configure interrupt setting for a specific pin. set on or off.
    pub async fn config_pin_interrupt(&self, pin: &Pin, enabled: mcp23017_rpi_lib::Feature, compare_mode: mcp23017_rpi_lib::Compare, defval: Option<State>) -> Result<(), DriverError>{
        let (tx, rx) = oneshot::channel();
        let command = MCP23017Command::ConfigPinInt(pin.clone(), enabled, compare_mode, defval, tx);
        
        self.send(command).await?;
        self.receive(rx).await
    }

    #[allow(dead_code)]
    // This function should be called when INTA or INTB is triggered to indicate an interrupt occurred.
    /// The function determines the pin that caused the interrupt and gets its value.
    /// The interrupt is cleared.
    /// Returns pin and the value.
    pub async fn read_interrupt(&self, port: mcp23017_rpi_lib::Bank) -> Result<Option<(Pin, State)>, DriverError> {
        let (tx, rx) = oneshot::channel();
        let command = MCP23017Command::ReadInt(port, tx);
        
        self.send(command).await?;
        self.receive(rx).await
    }

    #[allow(dead_code)]
    /// Check to see if there is an interrupt pending 3 times in a row (indicating it's stuck) 
    /// and if needed clear the interrupt without reading values.
    pub async fn clear_interrupts(&self) -> Result<(), DriverError> {
        let (tx, rx) = oneshot::channel();
        let command = MCP23017Command::ClearInt(tx);
        
        self.send(command).await?;
        self.receive(rx).await
    }

    /// Reset all pins and interrupts
    pub async fn reset(&self) -> Result<(), DriverError> {
        let (tx, rx) = oneshot::channel();
        let command = MCP23017Command::Reset(tx);
        
        self.send(command).await?;
        self.receive(rx).await
    }
}
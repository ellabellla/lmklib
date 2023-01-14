
use std::{sync::{Arc}, collections::HashSet, hash::Hash};

use async_trait::async_trait;
use key_module::Data;
use serde::{Serialize, Deserialize};
use tokio::sync::RwLock;
use virt_hid::{key::{SpecialKey, Modifier, BasicKey}, mouse::{MouseDir}};
use crate::{layout::{Layout}, OrLogIgnore, driver::DriverManager, modules::{ModuleManager, ExternalFunction}};

/// Keyboard functions
pub mod keyboard;
/// Mouse functions
pub mod mouse;
/// Midi functions
pub mod midi;
/// Command functions
pub mod cmd;
/// HID function controller
pub mod hid;
/// Log functions
pub mod log;
/// NanoMsg functions
pub mod nng;
/// Output functions
pub mod output;

use self::{keyboard::{Key, BasicString, ComplexString, Special, Shortcut, ModifierKey}, mouse::{ConstMove, LeftClick, RightClick, ConstScroll, Move, Scroll, ImmediateMove, ImmediateScroll}, midi::{Note, MidiController, Channel, ConstPitchBend, PitchBend, Instrument, GMSoundSet, note_param}, cmd::{Bash, Pipe, CommandPool}, hid::{HID, SwitchHid}, log::{Log, LogLevel}, nng::{DriverData, NanoMsg, NanoMessenger}, output::{Output, Flip}};


#[derive(Debug, Clone, Serialize, Deserialize, Eq)]
/// Function controller configuration data types, used for serialization
pub enum FunctionConfigData {
    CommandPool,
    HID { mouse: String, keyboard: String, led: String },
    MidiController,
    NanoMsg { pub_addr: String, sub_addr: String, timeout: i64 },
}

impl Hash for FunctionConfigData {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        core::mem::discriminant(self).hash(state);
    }
}

impl PartialEq for FunctionConfigData {
    fn eq(&self, other: &Self) -> bool {
        core::mem::discriminant(self) == core::mem::discriminant(other)
    }
}

#[async_trait]
/// Function config interface, used to serialize function controller data
pub trait FunctionConfig {
    type Output;
    type Error;
    fn to_config_data(&self) -> FunctionConfigData;
    async fn from_config(function_config: &FunctionConfiguration) -> Result<Self::Output, Self::Error>;
}

/// Function configuration, managers function controller configs
pub struct FunctionConfiguration {
    module_manager: Arc<ModuleManager>,
    configs: HashSet<FunctionConfigData>,
}

impl FunctionConfiguration {
    /// New
    pub fn new(config: &str, module_manager: Arc<ModuleManager>) -> Result<FunctionConfiguration, serde_json::Error> {
        let configs = serde_json::from_str(config)?;
        Ok(FunctionConfiguration { configs, module_manager })
    }

    /// Create new config data
    pub fn create_config() -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(&HashSet::<FunctionConfigData>::new())
    }

    #[allow(dead_code)]
    /// Insert configuration
    pub fn insert(&mut self, config: FunctionConfigData) -> bool {
        self.configs.insert(config)
    }

    /// Get first configuration where matches returns true 
    pub fn get<M>(&self, matches: M) -> Option<&FunctionConfigData> 
    where 
        M: FnMut(&&FunctionConfigData) -> bool
    {
        self.configs.iter().find(matches)
    }
}

/// Function return type
pub enum ReturnCommand {
    /// Switch layout
    Switch(usize),
    /// Up layout
    Up,
    /// Down layout
    Down,
    /// Return
    None,
}

impl ReturnCommand {
    /// Evaluation return
    pub fn eval(&self, layout: &mut Layout) {
        match self {
            ReturnCommand::Switch(index) => {layout.switch_layer(*index);},
            ReturnCommand::Up => {layout.up_layer();},
            ReturnCommand::Down => {layout.down_layer();},
            ReturnCommand::None => return,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
/// Function type, used for serializing functions
pub enum FunctionType {
    Key(char),
    ConstMove{x: i8, y: i8},
    Up,
    Down,
    Switch(usize),
    None,
    LeftClick,
    RightClick,
    ConstScroll{amount: i8, period: u64},
    String(String),
    ComplexString { str: String, layout: String },
    Special(SpecialKey),
    Shortcut { modifiers: Vec<Modifier>, keys: Vec<BasicKey> },
    Modifier(Modifier),
    StringLn(String),
    ComplexStringLn { str: String, layout: String },
    Move { dir: MouseDir, invert: bool, threshold: u16, scale: f64 },
    Scroll { period: u64, invert: bool, threshold: u16, scale: f64 },
    ImmediateMove { x: i8, y: i8 },
    ImmediateScroll(i8),
    Note{channel: Channel, note: note_param::Note, velocity: u8},
    ConstPitchBend{channel: Channel, bend: u16},
    PitchBend { channel: Channel, invert: bool, threshold: u16, scale: f64 },
    Instrument { channel: Channel, instrument: GMSoundSet },
    Bash(String),
    Pipe(String),
    SwitchHid{name: String},
    Log(LogLevel, String),
    NanoMsg { topic: u8, format: String, driver_data: Vec<DriverData> },
    External{ module: String, func: Data },
    Output { driver_name: String, idx: usize, state: u16 },
    Flip { driver_name: String, idx: usize },
}

impl FunctionType  {
    /// Get type from function
    pub fn from_function(f: &Function) -> Self {
        match f {
            Some(func) => func.ftype(),
            None => FunctionType::None,
        }
    }
}

/// Function builder
pub struct FunctionBuilder {
    hid: Arc<RwLock<HID>>,
    midi_controller: Arc<RwLock<MidiController>>,
    command_pool: Arc<RwLock<CommandPool>>,
    driver_manager: Arc<RwLock<DriverManager>>,
    nano_messenger: Arc<RwLock<NanoMessenger>>,
    module_manager: Arc<ModuleManager>,
}

impl FunctionBuilder {
    /// New
    pub fn new(
        hid: Arc<RwLock<HID>>, 
        midi_controller: Arc<RwLock<MidiController>>, 
        command_pool: Arc<RwLock<CommandPool>>, 
        driver_manager: Arc<RwLock<DriverManager>>,
        nano_messenger: Arc<RwLock<NanoMessenger>>,
        module_manager: Arc<ModuleManager>,
    ) -> Arc<RwLock<FunctionBuilder>> {
        Arc::new(RwLock::new(FunctionBuilder { hid, midi_controller, command_pool, driver_manager, nano_messenger, module_manager}))
    }


    /// Build function
    pub async fn build(&self, ftype: FunctionType) -> Function {
        let debug = format!("{:?}", ftype);
        match ftype {
            FunctionType::Key(key) => Key::new(key, self.hid.clone()),
            FunctionType::Special(special) => Special::new(special, self.hid.clone()),
            FunctionType::Modifier(modifier) => ModifierKey::new(modifier, self.hid.clone()),
            FunctionType::String(str) => BasicString::new(str, self.hid.clone()),
            FunctionType::ComplexString { str, layout } => ComplexString::new(str, layout, self.hid.clone()),
            FunctionType::StringLn(string) => BasicString::new(format!("{}\n", string), self.hid.clone()),
            FunctionType::ComplexStringLn { str, layout } => ComplexString::new(format!("{}\n", str), layout, self.hid.clone()),
            FunctionType::Shortcut { modifiers, keys } => Shortcut::new(modifiers, keys, self.hid.clone()),
            FunctionType::Up => Up::new(),
            FunctionType::Down => Down::new(),
            FunctionType::Switch(id) => Switch::new(id),
            FunctionType::Scroll { period, invert, threshold, scale } => Scroll::new(period, invert, threshold, scale, self.hid.clone()),
            FunctionType::Move { dir, invert, threshold, scale } => Move::new(dir, invert, threshold, scale, self.hid.clone()),
            FunctionType::ImmediateMove { x, y } => ImmediateMove::new(x, y, self.hid.clone()),
            FunctionType::ImmediateScroll(amount) => ImmediateScroll::new(amount, self.hid.clone()),
            FunctionType::ConstMove{x, y} => ConstMove::new(x, y, self.hid.clone()),
            FunctionType::ConstScroll{amount, period} => ConstScroll::new(amount, period, self.hid.clone()),
            FunctionType::LeftClick => LeftClick::new(self.hid.clone()),
            FunctionType::RightClick => RightClick::new(self.hid.clone()),
            FunctionType::None => None,
            FunctionType::Note{channel, note, velocity} => Note::new(channel, note, velocity, self.midi_controller.clone()),
            FunctionType::ConstPitchBend{channel, bend} => ConstPitchBend::new(channel, bend, self.midi_controller.clone()),
            FunctionType::PitchBend { channel, invert, threshold, scale } => PitchBend::new(channel, invert, threshold, scale, self.midi_controller.clone()),
            FunctionType::Instrument { channel, instrument } => Instrument::new(channel, instrument.into(), self.midi_controller.clone()),
            FunctionType::Bash(command) => Bash::new(command, self.command_pool.clone()),
            FunctionType::Pipe(command) => Pipe::new(command, self.command_pool.clone()),
            FunctionType::SwitchHid{name} => SwitchHid::new(name, self.hid.clone()),
            FunctionType::Log(log_level, msg) => Log::new(log_level, msg),
            FunctionType::NanoMsg { topic, format: msg, driver_data } => NanoMsg::new(topic, msg, driver_data, self.nano_messenger.clone(), self.driver_manager.clone()),
            FunctionType::External { module, func } => ExternalFunction::new(module, self.module_manager.clone(), func).await,
            FunctionType::Output { driver_name, idx, state } => Output::new(driver_name, idx, state, self.driver_manager.clone()),
            FunctionType::Flip { driver_name, idx } => Flip::new(driver_name, idx, self.driver_manager.clone()),
        }.or_log_ignore(&format!("Unable to build function (Function Builder), {}", debug))
    }
}

#[async_trait]
/// Function Interface
pub trait FunctionInterface {
    /// State poll event
    async fn event(&mut self, state: u16) -> ReturnCommand;
    /// Function Type
    fn ftype(&self) -> FunctionType;
}   

/// Function Object
pub type Function = Option<Box<dyn FunctionInterface + Send + Sync>>;

/// Up function
pub struct Up;

impl Up {
    /// New
    pub fn new() -> Function {
        Some(Box::new(Up))
    }
}

#[async_trait]
impl FunctionInterface for Up {
    async fn event(&mut self, _state: u16) -> ReturnCommand {
        return ReturnCommand::Up
    }

    fn ftype(&self) -> FunctionType {
        FunctionType::Up
    }
}

/// Down function
pub struct Down;

impl Down {
    /// New
    pub fn new() -> Function {
        Some(Box::new(Down))
    }
}

#[async_trait]
impl FunctionInterface for Down {
    async fn event(&mut self, _state: u16) -> ReturnCommand {
        return ReturnCommand::Down
    }

    fn ftype(&self) -> FunctionType {
        FunctionType::Down
    }
}

/// Switch function
pub struct Switch {
    id: usize
}

impl Switch {
    /// New
    pub fn new(id: usize) -> Function {
        Some(Box::new(Switch{id}))
    }
}

#[async_trait]
impl FunctionInterface for Switch {
    async fn event(&mut self, _state: u16) -> ReturnCommand {
        return ReturnCommand::Switch(self.id)
    }

    fn ftype(&self) -> FunctionType {
        FunctionType::Switch(self.id)
    }
}
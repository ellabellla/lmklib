
use std::{sync::{Arc}, collections::HashSet, hash::Hash};

use configfs::async_trait;
use serde::{Serialize, Deserialize};
use tokio::sync::RwLock;
use virt_hid::{key::{SpecialKey, Modifier, BasicKey}, mouse::{MouseDir}};
use crate::{layout::{Layout}, OrLogIgnore, driver::DriverManager};

pub mod keyboard;
pub mod mouse;
pub mod midi;
pub mod cmd;
pub mod hid;
pub mod log;
pub mod nng;

use self::{keyboard::{Key, BasicString, ComplexString, Special, Shortcut, ModifierKey}, mouse::{ConstMove, LeftClick, RightClick, ConstScroll, Move, Scroll, ImmediateMove, ImmediateScroll}, midi::{Note, MidiController, Channel, ConstPitchBend, PitchBend, Instrument, GMSoundSet, note_param}, cmd::{Bash, Pipe, CommandPool}, hid::{HID, SwitchHid}, log::{Log, LogLevel}, nng::{DriverData, NanoMsg, NanoMessenger}};


#[derive(Debug, Clone, Serialize, Deserialize, Eq)]
pub enum FunctionConfigData {
    CommandPool,
    HID { mouse_id: u8, keyboard_id: u8 },
    MidiController,
    NanoMsg { addresses: Vec<String>, timeout: i64 },
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
pub trait FunctionConfig {
    type Output;
    type Error;
    fn to_config_data(&self) -> FunctionConfigData;
    async fn from_config(function_config: &FunctionConfiguration) -> Result<Self::Output, Self::Error>;
}

#[derive(Debug)]
pub struct FunctionConfiguration {
    configs: HashSet<FunctionConfigData>,
}

impl FunctionConfiguration {
    pub fn new() -> FunctionConfiguration {
        FunctionConfiguration { configs: HashSet::new() }
    }

    #[allow(dead_code)]
    pub fn insert(&mut self, config: FunctionConfigData) -> bool {
        self.configs.insert(config)
    }

    pub fn get<M>(&self, matches: M) -> Option<&FunctionConfigData> 
    where 
        M: FnMut(&&FunctionConfigData) -> bool
    {
        self.configs.iter().find(matches)
    }
}

impl Serialize for FunctionConfiguration {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer 
    {
        self.configs.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for FunctionConfiguration {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de> 
    {
        let configs = HashSet::<FunctionConfigData>::deserialize(deserializer)?;    
        Ok(FunctionConfiguration { configs })
    }
}

pub enum ReturnCommand {
    Switch(usize),
    Up,
    Down,
    None,
}

impl ReturnCommand {
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
    SwitchHid,
    Log(LogLevel, String),
    NanoMsg { msg: String, driver_data: Vec<DriverData> },
}

impl FunctionType  {
    pub fn from_function(f: &Function) -> Self {
        match f {
            Some(func) => func.ftype(),
            None => FunctionType::None,
        }
    }
}

pub struct FunctionBuilder {
    hid: Arc<RwLock<HID>>,
    midi_controller: Arc<RwLock<MidiController>>,
    command_pool: Arc<RwLock<CommandPool>>,
    driver_manager: Arc<RwLock<DriverManager>>,
    nano_messenger: Arc<RwLock<NanoMessenger>>,
}

impl FunctionBuilder {
    pub fn new(
        hid: Arc<RwLock<HID>>, 
        midi_controller: Arc<RwLock<MidiController>>, 
        command_pool: Arc<RwLock<CommandPool>>, 
        driver_manager: Arc<RwLock<DriverManager>>,
        nano_messenger: Arc<RwLock<NanoMessenger>>,
    ) -> Arc<RwLock<FunctionBuilder>> {
        Arc::new(RwLock::new(FunctionBuilder { hid, midi_controller, command_pool, driver_manager, nano_messenger}))
    }

    pub fn build(&self, ftype: FunctionType) -> Function {
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
            FunctionType::SwitchHid => SwitchHid::new(self.hid.clone()),
            FunctionType::Log(log_level, msg) => Log::new(log_level, msg),
            FunctionType::NanoMsg { msg, driver_data } => NanoMsg::new(msg, driver_data, self.nano_messenger.clone(), self.driver_manager.clone()),
        }.or_log_ignore(&format!("Unable to build function (Function Builder), {}", debug))
    }
}

#[async_trait]
pub trait FunctionInterface {
    async fn event(&mut self, state: u16) -> ReturnCommand;
    fn ftype(&self) -> FunctionType;
}   


pub type Function = Option<Box<dyn FunctionInterface + Send + Sync>>;

pub struct Up;

impl Up {
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

pub struct Down;

impl Down {
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

pub struct Switch {
    id: usize
}

impl Switch {
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
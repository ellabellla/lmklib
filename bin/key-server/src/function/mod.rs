
use std::{sync::{Arc}, collections::HashSet, hash::Hash};

use async_trait::async_trait;
use key_module::Data;
use serde::{Serialize, Deserialize};
use tokio::sync::RwLock;
use virt_hid::{key::{SpecialKey, Modifier, BasicKey}, mouse::{MouseDir}};
use crate::{layout::{Layout, self, Variable, Variables}, OrLogIgnore, driver::DriverManager, modules::{ModuleManager, ExternalFunction}};

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

const HALF_U16: u16 = u16::MAX / 2;

pub type State = u16;
pub trait StateHelpers {
    fn high(&self) -> bool;

    fn low(&self) -> bool;

    fn rising(&self, prev_state: Self) -> bool;

    fn falling(&self, prev_state: Self) -> bool;
}

impl StateHelpers  for State {
    fn high(&self) -> bool {
        return *self > HALF_U16
    }

    fn low(&self) -> bool {
        return *self <= HALF_U16
    }

    fn rising(&self, prev_state: Self) -> bool {
        return self.high() && prev_state.low()
    }

    fn falling(&self, prev_state: Self) -> bool {
        return self.low() && prev_state.high()
    }
}


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
    /// Shift layout too
    Shift(usize),
    // Return from shifted layout
    UnShift(usize),
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
            ReturnCommand::Shift(index) => {layout.shift(*index);},
            ReturnCommand::UnShift(index) => {layout.unshift(*index);},
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
/// Function type, used for serializing functions
pub enum FunctionType {
    Key(char),
    ConstMove{x: layout::Data<i8>, y: layout::Data<i8>},
    Up,
    Down,
    Switch(layout::Data<usize>),
    Shift(layout::Data<usize>),
    None,
    LeftClick,
    RightClick,
    ConstScroll{amount: layout::Data<i8>, period: layout::Data<u64>},
    String(layout::Data<String>),
    ComplexString { str: layout::Data<String>, layout: layout::Data<String> },
    Special(SpecialKey),
    Shortcut { modifiers: Vec<Modifier>, keys: Vec<BasicKey> },
    Modifier(Modifier),
    StringLn(layout::Data<String>),
    ComplexStringLn { str: layout::Data<String>, layout: layout::Data<String> },
    Move { dir: MouseDir, invert: layout::Data<bool>, threshold: layout::Data<u16>, scale: layout::Data<f64> },
    Scroll { period: layout::Data<u64>, invert: layout::Data<bool>, threshold: layout::Data<u16>, scale: layout::Data<f64> },
    ImmediateMove { x: layout::Data<i8>, y: layout::Data<i8> },
    ImmediateScroll(layout::Data<i8>),
    Note{channel: layout::Data<Channel>, note: layout::Data<note_param::Note>, velocity: layout::Data<u8>},
    ConstPitchBend{channel: layout::Data<Channel>, bend: layout::Data<u16>},
    PitchBend { channel: layout::Data<Channel>, invert: layout::Data<bool>, threshold: layout::Data<u16>, scale: layout::Data<f64> },
    Instrument { channel: layout::Data<Channel>, instrument: layout::Data<GMSoundSet> },
    Bash(layout::Data<String>),
    Pipe(layout::Data<String>),
    SwitchHid{name: layout::Data<String>},
    Log(layout::Data<LogLevel>, layout::Data<String>),
    NanoMsg { topic: u8, format: String, driver_data: Vec<DriverData> },
    External{ module: String, func: Data },
    Output { driver_name: String, idx: layout::Data<usize>, state: layout::Data<u16> },
    Flip { driver_name: String, idx: layout::Data<usize>},
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
    variables: Arc<RwLock<Variables>>,
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
        variables: Arc<RwLock<Variables>>,
    ) -> Arc<RwLock<FunctionBuilder>> {
        Arc::new(RwLock::new(FunctionBuilder { hid, midi_controller, command_pool, driver_manager, nano_messenger, module_manager, variables}))
    }


    /// Build function
    pub async fn build(&self, ftype: FunctionType) -> Function {
        let debug = format!("{:?}", ftype);
        match ftype {
            FunctionType::Key(key) => Key::new(key, self.hid.clone()),
            FunctionType::Special(special) => Special::new(special, self.hid.clone()),
            FunctionType::Modifier(modifier) => ModifierKey::new(modifier, self.hid.clone()),
            FunctionType::String(str) => BasicString::new(str.into_variable(self.variables.clone()), false, self.hid.clone(), self.variables.clone()),
            FunctionType::ComplexString { str, layout } => ComplexString::new(str.into_variable(self.variables.clone()), false, layout.into_variable(self.variables.clone()), self.hid.clone(), self.variables.clone()),
            FunctionType::StringLn(string) => BasicString::new( string.into_variable(self.variables.clone()), true, self.hid.clone(), self.variables.clone()),
            FunctionType::ComplexStringLn { str, layout } => ComplexString::new(str.into_variable(self.variables.clone()), true, layout.into_variable(self.variables.clone()), self.hid.clone(), self.variables.clone()),
            FunctionType::Shortcut { modifiers, keys } => Shortcut::new(modifiers, keys, self.hid.clone()),
            FunctionType::Up => Up::new(),
            FunctionType::Down => Down::new(),
            FunctionType::Switch(id) => Switch::new(id.into_variable(self.variables.clone())),
            FunctionType::Scroll { period, invert, threshold, scale } => Scroll::new(period.into_variable(self.variables.clone()), invert.into_variable(self.variables.clone()), threshold.into_variable(self.variables.clone()), scale.into_variable(self.variables.clone()), self.hid.clone(), self.variables.clone()),
            FunctionType::Move { dir, invert, threshold, scale } => Move::new(dir, invert.into_variable(self.variables.clone()), threshold.into_variable(self.variables.clone()), scale.into_variable(self.variables.clone()), self.hid.clone()),
            FunctionType::ImmediateMove { x, y } => ImmediateMove::new(x.into_variable(self.variables.clone()), y.into_variable(self.variables.clone()), self.hid.clone()),
            FunctionType::ImmediateScroll(amount) => ImmediateScroll::new(amount.into_variable(self.variables.clone()), self.hid.clone()),
            FunctionType::ConstMove{x, y} => ConstMove::new(x.into_variable(self.variables.clone()), y.into_variable(self.variables.clone()), self.hid.clone()),
            FunctionType::ConstScroll{amount, period} => ConstScroll::new(amount.into_variable(self.variables.clone()), period.into_variable(self.variables.clone()), self.hid.clone(), self.variables.clone()),
            FunctionType::LeftClick => LeftClick::new(self.hid.clone()),
            FunctionType::RightClick => RightClick::new(self.hid.clone()),
            FunctionType::None => None,
            FunctionType::Note{channel, note, velocity} => Note::new(channel.into_variable(self.variables.clone()), note.into_variable(self.variables.clone()), velocity.into_variable(self.variables.clone()), self.midi_controller.clone(), self.variables.clone()),
            FunctionType::ConstPitchBend{channel, bend} => ConstPitchBend::new(channel.into_variable(self.variables.clone()), bend.into_variable(self.variables.clone()), self.midi_controller.clone(), self.variables.clone()),
            FunctionType::PitchBend { channel, invert, threshold, scale } => PitchBend::new(channel.into_variable(self.variables.clone()), invert.into_variable(self.variables.clone()), threshold.into_variable(self.variables.clone()), scale.into_variable(self.variables.clone()), self.midi_controller.clone(), self.variables.clone()),
            FunctionType::Instrument { channel, instrument } => Instrument::new(channel.into_variable(self.variables.clone()), instrument.into_variable(self.variables.clone()), self.midi_controller.clone(), self.variables.clone()),
            FunctionType::Bash(command) => Bash::new(command.into_variable(self.variables.clone()), self.command_pool.clone()),
            FunctionType::Pipe(command) => Pipe::new(command.into_variable(self.variables.clone()), self.command_pool.clone()),
            FunctionType::SwitchHid{name} => SwitchHid::new(name.into_variable(self.variables.clone()), self.hid.clone()),
            FunctionType::Log(log_level, msg) => Log::new(log_level.into_variable(self.variables.clone()), msg.into_variable(self.variables.clone())),
            FunctionType::NanoMsg { topic, format: msg, driver_data } => NanoMsg::new(topic, msg, driver_data, self.nano_messenger.clone(), self.driver_manager.clone()),
            FunctionType::External { module, func } => ExternalFunction::new(module, self.module_manager.clone(), func).await,
            FunctionType::Output { driver_name, idx, state } => Output::new(driver_name, idx.into_variable(self.variables.clone()), state.into_variable(self.variables.clone()), self.driver_manager.clone()),
            FunctionType::Flip { driver_name, idx } => Flip::new(driver_name, idx.into_variable(self.variables.clone()), self.driver_manager.clone()),
            FunctionType::Shift(id) => Shift::new(id.into_variable(self.variables.clone())),
        }.or_log_ignore(&format!("Unable to build function (Function Builder), {}", debug))
    }
}

#[async_trait]
/// Function Interface
pub trait FunctionInterface {
    /// State poll event
    async fn event(&mut self, state: State) -> ReturnCommand;
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
    async fn event(&mut self, _state: State) -> ReturnCommand {
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
    async fn event(&mut self, _state: State) -> ReturnCommand {
        return ReturnCommand::Down
    }

    fn ftype(&self) -> FunctionType {
        FunctionType::Down
    }
}

/// Switch function
pub struct Switch {
    id: Variable<usize>,
    prev_state: u16,
}

impl Switch {
    /// New
    pub fn new(id: Variable<usize>) -> Function {
        Some(Box::new(Switch{id, prev_state: 0}))
    }
}

#[async_trait]
impl FunctionInterface for Switch {
    async fn event(&mut self, state: State) -> ReturnCommand {
        if state.rising(self.prev_state) {
            let mut lock = self.id.write_lock_variables().await;

            return ReturnCommand::Switch(**self.id.data(&mut lock))
        }

        return ReturnCommand::None
    }

    fn ftype(&self) -> FunctionType {
        FunctionType::Switch(self.id.into_data())
    }
}

/// Shift function
pub struct Shift {
    id: Variable<usize>,
    prev_state: u16,
}

impl Shift {
    /// New
    pub fn new(id: Variable<usize>) -> Function {
        Some(Box::new(Shift{id, prev_state: 0}))
    }
}

#[async_trait]
impl FunctionInterface for Shift {
    async fn event(&mut self, state: State) -> ReturnCommand {
        let mut lock = self.id.write_lock_variables().await;
        if state.rising(self.prev_state) {
            return ReturnCommand::Shift(**self.id.data(&mut lock))
        } else if state.falling(self.prev_state) {
            return ReturnCommand::UnShift(**self.id.data(&mut lock))
        }

        ReturnCommand::None
    }

    fn ftype(&self) -> FunctionType {
        FunctionType::Shift(self.id.into_data())
    }
}
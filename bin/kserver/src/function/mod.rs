
use std::{sync::{Arc}, io};

use serde::{Serialize, Deserialize};
use tokio::sync::RwLock;
use virt_hid::{key::{Keyboard, SpecialKey, Modifier, BasicKey}, mouse::{Mouse, MouseDir}};
use crate::layout::{Layout};

pub mod keyboard;
pub mod mouse;
pub mod midi;

use self::{keyboard::{Key, BasicString, ComplexString, Special, Shortcut, ModifierKey}, mouse::{ConstMove, LeftClick, RightClick, ConstScroll, Move, Scroll, ImmediateMove, ImmediateScroll}, midi::{Note, MidiController, Channel, ConstPitchBend, PitchBend}};

pub struct HID {
    pub(crate) keyboard: Keyboard,
    pub(crate) mouse: Mouse,
    hid: virt_hid::HID,
}

impl HID {
    pub fn new(mouse_id: u8, keyboard_id: u8) -> io::Result<HID> {
        Ok(HID { keyboard: Keyboard::new(), mouse: Mouse::new(), hid: virt_hid::HID::new(mouse_id, keyboard_id)? })
    }
    
    pub fn send_keyboard(&mut self) -> io::Result<()> {
        self.keyboard.send(&mut self.hid)
    }
    
    pub fn send_mouse(&mut self) -> io::Result<()> {
        self.mouse.send(&mut self.hid)
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

#[derive(Clone, Serialize, Deserialize)]
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
    Note{channel: Channel, freq: f32, velocity: u8},
    ConstPitchBend{channel: Channel, bend: u16},
    PitchBend { channel: Channel, invert: bool, threshold: u16, scale: f64 },
}

impl From<&Function> for  FunctionType  {
    fn from(f: &Function) -> Self {
        match f {
            Some(func) => func.ftype(),
            None => FunctionType::None,
        }
    }
}

pub struct FunctionBuilder {
    hid: Arc<RwLock<HID>>,
    midi_controller: Arc<RwLock<MidiController>>,
}

impl FunctionBuilder {
    pub fn new(hid: HID, midi_controller: MidiController) -> FunctionBuilder {
        FunctionBuilder { hid: Arc::new(RwLock::new(hid)), midi_controller: Arc::new(RwLock::new(midi_controller)) }
    }

    pub fn build(&self, ftype: FunctionType) -> Function {
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
            FunctionType::Note{channel, freq, velocity} => Note::new(channel, freq, velocity, self.midi_controller.clone()),
            FunctionType::ConstPitchBend{channel, bend} => ConstPitchBend::new(channel, bend, self.midi_controller.clone()),
            FunctionType::PitchBend { channel, invert, threshold, scale } => PitchBend::new(channel, invert, threshold, scale, self.midi_controller.clone()),
        }
    }
}

pub trait FunctionInterface {
    fn event(&mut self, state: u16) -> ReturnCommand;
    fn ftype(&self) -> FunctionType;
}   


pub type Function = Option<Box<dyn FunctionInterface + Send + Sync>>;

pub struct Up;

impl Up {
    pub fn new() -> Function {
        Some(Box::new(Up))
    }
}

impl FunctionInterface for Up {
    fn event(&mut self, _state: u16) -> ReturnCommand {
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

impl FunctionInterface for Down {
    fn event(&mut self, _state: u16) -> ReturnCommand {
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

impl FunctionInterface for Switch {
    fn event(&mut self, _state: u16) -> ReturnCommand {
        return ReturnCommand::Switch(self.id)
    }

    fn ftype(&self) -> FunctionType {
        FunctionType::Switch(self.id)
    }
}
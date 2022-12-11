
use std::{sync::{RwLock, Arc}, io};

use serde::{Serialize, Deserialize};
use virt_hid::{key::{Keyboard}, mouse::Mouse};
use crate::layout::Layout;

pub mod keyboard;
pub mod mouse;

use self::{keyboard::Key, mouse::{ConstMouse, LeftClick, RightClick, ConstWheel}};

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
    ConstMouse{x: i8, y: i8},
    Up,
    Down,
    Switch(usize),
    None,
    LeftClick,
    RightClick,
    ConstWheel{amount: i8, period: u64},
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
}

impl FunctionBuilder {
    pub fn new(hid: HID) -> FunctionBuilder {
        FunctionBuilder { hid: Arc::new(RwLock::new(hid)) }
    }

    pub fn build(&self, ftype: FunctionType) -> Function {
        match ftype {
            FunctionType::Key(char) => Some(Box::new(Key{
                key: char, 
                hid: self.hid.clone(), 
                prev_state: 0
            })),
            FunctionType::Up => Up::new(),
            FunctionType::Down => Down::new(),
            FunctionType::Switch(id) => Switch::new(id),
            FunctionType::ConstMouse{x, y} => ConstMouse::new(x, y, self.hid.clone()),
            FunctionType::ConstWheel{amount, period} => ConstWheel::new(amount, period, self.hid.clone()),
            FunctionType::LeftClick => LeftClick::new(self.hid.clone()),
            FunctionType::RightClick => RightClick::new(self.hid.clone()),
            FunctionType::None => None,
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
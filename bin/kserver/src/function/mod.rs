
use std::sync::{RwLock, Arc};

use serde::{Serialize, Deserialize};
use crate::layout::Layout;

pub mod keyboard;
use keyboard::KeyboardBundle;

use self::keyboard::Key;

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
    None,
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
    keyboard: Arc<RwLock<KeyboardBundle>>,
}

impl FunctionBuilder {
    pub fn new(keyboard: KeyboardBundle) -> FunctionBuilder {
        FunctionBuilder { keyboard: Arc::new(RwLock::new(keyboard)) }
    }

    pub fn build(&self, ftype: FunctionType) -> Function {
        match ftype {
            FunctionType::Key(char) => Some(Box::new(Key{
                key: char, 
                keyboard_bundle: self.keyboard.clone(), 
                prev_state: 0
            })),
            FunctionType::None => None,
        }
    }
}

pub trait FunctionInterface {
    fn event(&mut self, state: u16) -> ReturnCommand;
    fn ftype(&self) -> FunctionType;
}   


pub type Function = Option<Box<dyn FunctionInterface + Send + Sync>>;
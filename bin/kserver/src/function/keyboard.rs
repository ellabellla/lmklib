use std::{sync::{Arc}};

use tokio::sync::RwLock;
use virt_hid::key::{BasicKey, KeyOrigin, SpecialKey, Modifier};

use super::{FunctionInterface, ReturnCommand, FunctionType, HID, Function};


pub struct Key{
    key: char,
    hid: Arc<RwLock<HID>>,
    prev_state: u16,
}

impl Key {
    pub fn new(key: char, hid: Arc<RwLock<HID>>) -> Function {
        Some(Box::new(Key { key, prev_state: 0, hid }))
    }
}

impl FunctionInterface for Key {
    fn event(&mut self, state: u16) -> ReturnCommand {
        if state != 0 && self.prev_state == 0 {
            let mut hid = self.hid.blocking_write(); 

            hid.keyboard.hold_key(&BasicKey::Char(self.key, KeyOrigin::Keyboard));
            hid.send_keyboard().ok();
        } else if state == 0 && self.prev_state != 0{
            let mut hid = self.hid.blocking_write();
            
            hid.keyboard.release_key(&BasicKey::Char(self.key, KeyOrigin::Keyboard));
            hid.send_keyboard().ok();
        }

        self.prev_state = state;
        ReturnCommand::None
    }

    fn ftype(&self) -> FunctionType {
        FunctionType::Key(self.key)
    }
}

pub struct Special {
    special: SpecialKey,
    hid: Arc<RwLock<HID>>,
    prev_state: u16,
}

impl Special {
    pub fn new(special: SpecialKey, hid: Arc<RwLock<HID>>) -> Function {
        Some(Box::new(Special { special, prev_state: 0, hid }))
    }
}

impl FunctionInterface for Special {
    fn event(&mut self, state: u16) -> ReturnCommand {
        if state != 0 && self.prev_state == 0 {
            let mut hid = self.hid.blocking_write();

            hid.keyboard.hold_key(&BasicKey::Special(self.special));
            hid.send_keyboard().ok();
        } else if state == 0 && self.prev_state != 0{
            let mut hid = self.hid.blocking_write();
            
            hid.keyboard.release_key(&BasicKey::Special(self.special));
            hid.send_keyboard().ok();
        }

        self.prev_state = state;
        ReturnCommand::None
    }

    fn ftype(&self) -> FunctionType {
        FunctionType::Special(self.special)
    }
}

pub struct ModifierKey {
    modifier: Modifier,
    hid: Arc<RwLock<HID>>,
    prev_state: u16,
}

impl ModifierKey {
    pub fn new(modifier: Modifier, hid: Arc<RwLock<HID>>) -> Function {
        Some(Box::new(ModifierKey { modifier, prev_state: 0, hid }))
    }
}

impl FunctionInterface for ModifierKey {
    fn event(&mut self, state: u16) -> ReturnCommand {
        if state != 0 && self.prev_state == 0 {
            let mut hid = self.hid.blocking_write();

            hid.keyboard.hold_mod(&self.modifier);
            hid.send_keyboard().ok();
        } else if state == 0 && self.prev_state != 0 {
            let mut hid = self.hid.blocking_write();
            
            hid.keyboard.release_mod(&self.modifier);
            hid.send_keyboard().ok();
        }

        self.prev_state = state;
        ReturnCommand::None
    }

    fn ftype(&self) -> FunctionType {
        FunctionType::Modifier(self.modifier)
    }
}

pub struct BasicString {
    string: std::string::String,
    prev_state: u16,
    hid: Arc<RwLock<HID>>,
}

impl BasicString {
    pub fn new(string: std::string::String, hid: Arc<RwLock<HID>>) -> Function {
        Some(Box::new(BasicString { string, prev_state: 0, hid }))
    }
}

impl FunctionInterface for BasicString {
    fn event(&mut self, state: u16) -> ReturnCommand {
        if state != 0 && self.prev_state == 0 {
            let mut hid = self.hid.blocking_write();

            hid.keyboard.press_basic_string(&self.string);
            hid.send_keyboard().ok();
        }

        self.prev_state = state;
        ReturnCommand::None
    }

    fn ftype(&self) -> FunctionType {
        FunctionType::String(self.string.clone())
    }
}

pub struct ComplexString {
    string: std::string::String,
    layout: std::string::String,
    prev_state: u16,
    hid: Arc<RwLock<HID>>,
}

impl ComplexString {
    pub fn new(string: std::string::String, layout: std::string::String, hid: Arc<RwLock<HID>>) -> Function {
        Some(Box::new(ComplexString { string, layout, prev_state: 0, hid }))
    }
}

impl FunctionInterface for ComplexString {
    fn event(&mut self, state: u16) -> ReturnCommand {
        if state != 0 && self.prev_state == 0 {
            let mut hid = self.hid.blocking_write();

            hid.keyboard.press_string(&self.layout, &self.string);
            hid.send_keyboard().ok();
        }

        self.prev_state = state;
        ReturnCommand::None
    }

    fn ftype(&self) -> FunctionType {
        FunctionType::ComplexString{str: self.string.clone(), layout: self.layout.clone()}
    }
}

pub struct Shortcut {
    modifiers: Vec<Modifier>,
    keys: Vec<BasicKey>,
    prev_state: u16,
    hid: Arc<RwLock<HID>>,
}

impl Shortcut {
    pub fn new(modifiers: Vec<Modifier>, keys: Vec<BasicKey>, hid: Arc<RwLock<HID>>) -> Function {
        Some(Box::new(Shortcut { modifiers, keys, prev_state: 0, hid }))
    }
}

impl FunctionInterface for Shortcut {
    fn event(&mut self, state: u16) -> ReturnCommand {
        if state != 0 && self.prev_state == 0 {
            let mut hid = self.hid.blocking_write();

            for modifier in &self.modifiers {
                hid.keyboard.hold_mod(modifier);
            }
            for key in &self.keys {
                hid.keyboard.hold_key(key);
            }
            hid.send_keyboard().ok();
            for key in &self.keys {
                hid.keyboard.release_key(key);
            }
            for modifier in &self.modifiers {
                hid.keyboard.release_mod(modifier);
            }
            hid.send_keyboard().ok();
        }

        self.prev_state = state;
        ReturnCommand::None
    }

    fn ftype(&self) -> FunctionType {
        FunctionType::Shortcut{modifiers: self.modifiers.clone(), keys: self.keys.clone()}
    }
}
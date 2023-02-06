use std::{sync::{Arc}};

use async_trait::async_trait;
use tokio::{sync::RwLock};
use virt_hid::key::{BasicKey, SpecialKey, Modifier};

use crate::variables::{Variable};

use super::{FunctionInterface, ReturnCommand, FunctionType, hid::HID, Function, State, StateHelpers};

/// Key function (acts as normal key)
pub struct Key{
    key: char,
    hid: Arc<RwLock<HID>>,
    prev_state: u16,
}

impl Key {
    /// New
    pub fn new(key: char, hid: Arc<RwLock<HID>>) -> Function {
        Some(Box::new(Key { key, prev_state: 0, hid }))
    }
}

#[async_trait]
impl FunctionInterface for Key {
    async fn event(&mut self, state: State) -> ReturnCommand {
        if state.rising(self.prev_state) {
            let hid = self.hid.read().await; 

            hid.hold_key(self.key).await;
            hid.send_keyboard();
        } else if state.falling(self.prev_state) {
            let hid = self.hid.read().await;
            
            hid.release_key(self.key).await;
            hid.send_keyboard();
        }

        self.prev_state = state;
        ReturnCommand::None
    }

    fn ftype(&self) -> FunctionType {
        FunctionType::Key(self.key)
    }
}

/// Special Key function (acts as normal key)
pub struct Special {
    special: SpecialKey,
    hid: Arc<RwLock<HID>>,
    prev_state: u16,
}

impl Special {
    /// New
    pub fn new(special: SpecialKey, hid: Arc<RwLock<HID>>) -> Function {
        Some(Box::new(Special { special, prev_state: 0, hid }))
    }
}

#[async_trait]
impl FunctionInterface for Special {
    async fn event(&mut self, state: State) -> ReturnCommand {
        if state.rising(self.prev_state) {
            let hid = self.hid.read().await;

            hid.hold_special(self.special).await;
            hid.send_keyboard();
        } else if state.falling(self.prev_state) {
            let hid = self.hid.read().await;
            
            hid.release_special(self.special).await;
            hid.send_keyboard();
        }

        self.prev_state = state;
        ReturnCommand::None
    }

    fn ftype(&self) -> FunctionType {
        FunctionType::Special(self.special)
    }
}

/// Modifier key function (acts as normal key)
pub struct ModifierKey {
    modifier: Modifier,
    hid: Arc<RwLock<HID>>,
    prev_state: u16,
}

impl ModifierKey {
    /// New
    pub fn new(modifier: Modifier, hid: Arc<RwLock<HID>>) -> Function {
        Some(Box::new(ModifierKey { modifier, prev_state: 0, hid }))
    }
}

#[async_trait]
impl FunctionInterface for ModifierKey {
    async fn event(&mut self, state: State) -> ReturnCommand {
        if state.rising(self.prev_state) {
            let hid = self.hid.read().await;

            hid.hold_mod(self.modifier).await;
            hid.send_keyboard();
        } else if state.falling(self.prev_state) {
            let hid = self.hid.read().await;
            
            hid.release_mod(self.modifier).await;
            hid.send_keyboard();
        }

        self.prev_state = state;
        ReturnCommand::None
    }

    fn ftype(&self) -> FunctionType {
        FunctionType::Modifier(self.modifier)
    }
}

/// Basic String function, types a string
pub struct BasicString {
    string: Variable<String>,
    prev_state: u16,
    hid: Arc<RwLock<HID>>,
}

impl BasicString {
    /// New
    pub fn new(mut string: Variable<String>, nl: bool, hid: Arc<RwLock<HID>>) -> Function {
        if nl {
            string = string.map(|s| format!("{}\n", s));
        }
        Some(Box::new(BasicString { string, prev_state: 0, hid }))
    }
}

#[async_trait]
impl FunctionInterface for BasicString {
    async fn event(&mut self, state: State) -> ReturnCommand {
        if state.rising(self.prev_state) {
            let hid = self.hid.read().await;

            hid.press_basic_string(self.string.data()).await;
            hid.send_keyboard();
        }

        self.prev_state = state;
        ReturnCommand::None
    }

    fn ftype(&self) -> FunctionType {
        FunctionType::String(self.string.into_data())
    }
}

/// Complex string function, types a string based on a keyboard layout
pub struct ComplexString {
    string: Variable<String>,
    layout: Variable<String>,
    prev_state: u16,
    hid: Arc<RwLock<HID>>,
}

impl ComplexString {
    /// New
    pub fn new(mut string: Variable<String>, nl: bool, layout: Variable<String>, hid: Arc<RwLock<HID>>) -> Function {
        if nl {
            string = string.map(|s| format!("{}\n", s));
        }
        Some(Box::new(ComplexString { string, layout, prev_state: 0, hid }))
    }
}

#[async_trait]
impl FunctionInterface for ComplexString {
    async fn event(&mut self, state: State) -> ReturnCommand {
        if state.rising(self.prev_state) {
            let hid = self.hid.read().await;

            hid.press_string(self.layout.data(), self.string.data()).await;
            hid.send_keyboard();
        }

        self.prev_state = state;
        ReturnCommand::None
    }

    fn ftype(&self) -> FunctionType {
        FunctionType::ComplexString{str: self.string.into_data(), layout: self.layout.into_data()}
    }
}

/// Shortcut function, presses a key combination (will not hold the combination)
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

#[async_trait]
impl FunctionInterface for Shortcut {
    async fn event(&mut self, state: State) -> ReturnCommand {
        if state.rising(self.prev_state) {
            let hid = self.hid.read().await;

            for modifier in &self.modifiers {
                hid.hold_mod(*modifier).await;
            }
            for key in &self.keys {
                match key {
                    BasicKey::Char(key, _) => hid.hold_key(*key).await,
                    BasicKey::Special(special) => hid.hold_special(*special).await,
                };
            }
            hid.send_keyboard();
            for key in &self.keys {
                match key {
                    BasicKey::Char(key, _) => hid.release_key(*key).await,
                    BasicKey::Special(special) => hid.release_special(*special).await,
                };
            }
            for modifier in &self.modifiers {
                hid.release_mod(*modifier).await;
            }
            hid.send_keyboard();
        }

        self.prev_state = state;
        ReturnCommand::None
    }

    fn ftype(&self) -> FunctionType {
        FunctionType::Shortcut{modifiers: self.modifiers.clone(), keys: self.keys.clone()}
    }
}
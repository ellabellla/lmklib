use std::{sync::{Arc, RwLock}, io};

use virt_hid::{key::{BasicKey, Keyboard, KeyOrigin}, HID};

use super::{FunctionInterface, ReturnCommand, FunctionType};

pub struct KeyboardBundle {
    keyboard: Keyboard,
    hid: HID,
}

impl KeyboardBundle {
    pub fn new(keyboard: Keyboard, hid: HID) -> KeyboardBundle {
        KeyboardBundle { keyboard, hid }
    }
    
    pub fn send(&mut self) -> io::Result<()> {
        self.keyboard.send(&mut self.hid)
    }
}

pub struct Key{
    pub(crate) key: char,
    pub(crate) keyboard_bundle: Arc<RwLock<KeyboardBundle>>,

    pub(crate) prev_state: u16,
}

impl FunctionInterface for Key {
    fn event(&mut self, state: u16) -> ReturnCommand {
        'block: {
            if state != 0 && self.prev_state == 0 {
                let Ok(mut bundle) = self.keyboard_bundle.write() else {
                    break 'block;
                };

                bundle.keyboard.hold_key(&BasicKey::Char(self.key, KeyOrigin::Keyboard));
                bundle.send().ok();
            } else if state == 0 && self.prev_state != 0{
                let Ok(mut bundle) = self.keyboard_bundle.write() else {
                    break 'block;
                };
                
                bundle.keyboard.release_key(&BasicKey::Char(self.key, KeyOrigin::Keyboard));
                bundle.send().ok();
            }
        }

        self.prev_state = state;
        ReturnCommand::None
    }

    fn ftype(&self) -> FunctionType {
        FunctionType::Key(self.key)
    }
}

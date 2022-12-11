use std::{sync::{Arc, RwLock}};

use virt_hid::key::{BasicKey, KeyOrigin};

use super::{FunctionInterface, ReturnCommand, FunctionType, HID};


pub struct Key{
    pub(crate) key: char,
    pub(crate) hid: Arc<RwLock<HID>>,

    pub(crate) prev_state: u16,
}

impl FunctionInterface for Key {
    fn event(&mut self, state: u16) -> ReturnCommand {
        'block: {
            if state != 0 && self.prev_state == 0 {
                let Ok(mut hid) = self.hid.write() else {
                    break 'block;
                };

                hid.keyboard.hold_key(&BasicKey::Char(self.key, KeyOrigin::Keyboard));
                hid.send_keyboard().ok();
            } else if state == 0 && self.prev_state != 0{
                let Ok(mut hid) = self.hid.write() else {
                    break 'block;
                };
                
                hid.keyboard.release_key(&BasicKey::Char(self.key, KeyOrigin::Keyboard));
                hid.send_keyboard().ok();
            }
        }

        self.prev_state = state;
        ReturnCommand::None
    }

    fn ftype(&self) -> FunctionType {
        FunctionType::Key(self.key)
    }
}

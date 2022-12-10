use std::sync::{Arc, RwLock};

use virt_hid::mouse::MouseDir;

use super::{FunctionInterface, HID, ReturnCommand, FunctionType, Function};


pub struct ConstMouse {
    amount: (i8, i8),
    hid: Arc<RwLock<HID>>
}

impl ConstMouse {
    pub fn new(x: i8, y: i8, hid: Arc<RwLock<HID>>) -> Function {
        Some(Box::new(ConstMouse{amount: (x, y), hid}))
    }
}

impl FunctionInterface for ConstMouse {
    fn event(&mut self, state: u16) -> super::ReturnCommand {
        let Ok(mut hid) = self.hid.write() else {
            return ReturnCommand::None;
        };

        if state != 0 {
            hid.mouse.move_mouse(&self.amount.0, &MouseDir::X);
            hid.mouse.move_mouse(&self.amount.1, &MouseDir::Y);

            hid.send().ok();
        }
        
        return super::ReturnCommand::None
    }

    fn ftype(&self) -> FunctionType {
        return FunctionType::ConstMouse(self.amount.0, self.amount.1)
    }
}
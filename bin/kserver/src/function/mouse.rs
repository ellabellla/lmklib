use std::{sync::{Arc, RwLock}, time::{Instant, Duration}};

use virt_hid::mouse::{MouseDir, MouseButton};

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

            hid.send_mouse().ok();
        }
        
        return super::ReturnCommand::None
    }

    fn ftype(&self) -> FunctionType {
        return FunctionType::ConstMouse{x: self.amount.0, y: self.amount.1}
    }
}
pub struct ConstWheel {
    amount: i8,
    period: Duration,
    prev_time: Instant,
    hid: Arc<RwLock<HID>>
}

impl ConstWheel {
    pub fn new(amount: i8, period: u64, hid: Arc<RwLock<HID>>) -> Function {
        Some(Box::new(ConstWheel{amount, period: Duration::from_millis(period), hid, prev_time: Instant::now()}))
    }
}

impl FunctionInterface for ConstWheel {
    fn event(&mut self, state: u16) -> super::ReturnCommand {

        let Ok(mut hid) = self.hid.write() else {
            return ReturnCommand::None;
        };

        if state != 0 {
            let now = Instant::now();
            if now.duration_since(self.prev_time) > self.period {
                self.prev_time = now;

                hid.mouse.scroll_wheel(&self.amount);
    
                hid.send_mouse().ok();
            }
        }
        
        return super::ReturnCommand::None
    }

    fn ftype(&self) -> FunctionType {
        return FunctionType::ConstWheel{amount: self.amount, period: self.period.as_millis() as u64}
    }
}

pub struct LeftClick {
    hid: Arc<RwLock<HID>>,
    prev_state: u16,
}

impl LeftClick {
    pub fn new(hid: Arc<RwLock<HID>>) -> Function {
        Some(Box::new(LeftClick{hid, prev_state: 0}))
    }
}

impl FunctionInterface for LeftClick {
    fn event(&mut self, state: u16) -> super::ReturnCommand {
        'block: {
            let Ok(mut hid) = self.hid.write() else {
                break 'block;
            };

            if state != 0 && self.prev_state == 0 {
                hid.mouse.hold_button(&MouseButton::Left);
                hid.send_mouse().ok();
            } else if state == 0 && self.prev_state != 0 {
                hid.mouse.release_button(&MouseButton::Left);
                hid.send_mouse().ok();
            }
        }

        self.prev_state = state;

        return super::ReturnCommand::None
    }

    fn ftype(&self) -> FunctionType {
        return FunctionType::LeftClick
    }
}

pub struct RightClick {
    hid: Arc<RwLock<HID>>,
    prev_state: u16,
}

impl RightClick {
    pub fn new(hid: Arc<RwLock<HID>>) -> Function {
        Some(Box::new(RightClick{hid, prev_state: 0}))
    }
}

impl FunctionInterface for RightClick {
    fn event(&mut self, state: u16) -> super::ReturnCommand {
        'block: {
            let Ok(mut hid) = self.hid.write() else {
                break 'block;
            };

            if state != 0 && self.prev_state == 0 {
                hid.mouse.hold_button(&MouseButton::Right);
                hid.send_mouse().ok();
            } else if state == 0 && self.prev_state != 0 {
                hid.mouse.release_button(&MouseButton::Right);
                hid.send_mouse().ok();
            }
        }

        self.prev_state = state;

        return super::ReturnCommand::None
    }

    fn ftype(&self) -> FunctionType {
        return FunctionType::RightClick
    }
}
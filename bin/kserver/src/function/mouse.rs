use std::{sync::{Arc, RwLock}, time::{Instant, Duration}};

use virt_hid::mouse::{MouseDir, MouseButton};

use super::{FunctionInterface, HID, ReturnCommand, FunctionType, Function};

pub struct ImmediateMove {
    amount: (i8, i8),
    prev_state: u16,
    hid: Arc<RwLock<HID>>
}

impl ImmediateMove {
    pub fn new(x: i8, y: i8, hid: Arc<RwLock<HID>>) -> Function {
        Some(Box::new(ImmediateMove{amount: (x, y), prev_state: 0, hid}))
    }
}

impl FunctionInterface for ImmediateMove {
    fn event(&mut self, state: u16) -> super::ReturnCommand {
        'block: {
            if state != 0 && self.prev_state == 0 {
                let Ok(mut hid) = self.hid.write() else {
                    break 'block;
                };

                hid.mouse.move_mouse(&self.amount.0, &MouseDir::X);
                hid.mouse.move_mouse(&self.amount.1, &MouseDir::Y);
    
                hid.send_mouse().ok();
            }
        }

        self.prev_state = state;
        ReturnCommand::None
    }

    fn ftype(&self) -> FunctionType {
        return FunctionType::ImmediateMove{ x: self.amount.0, y: self.amount.1}
    }
}
pub struct ImmediateScroll {
    amount: i8,
    prev_state: u16,
    hid: Arc<RwLock<HID>>
}

impl ImmediateScroll {
    pub fn new(amount: i8, hid: Arc<RwLock<HID>>) -> Function {
        Some(Box::new(ImmediateScroll{amount, prev_state: 0, hid}))
    }
}

impl FunctionInterface for ImmediateScroll {
    fn event(&mut self, state: u16) -> super::ReturnCommand {
        'block: {
            if state != 0 && self.prev_state == 0 {
                let Ok(mut hid) = self.hid.write() else {
                    break 'block;
                };

                hid.mouse.scroll_wheel(&self.amount);
                hid.send_mouse().ok();
            }
        }

        self.prev_state = state;
        ReturnCommand::None
    }

    fn ftype(&self) -> FunctionType {
        return FunctionType::ImmediateScroll(self.amount)
    }
}

pub struct ConstMove {
    amount: (i8, i8),
    hid: Arc<RwLock<HID>>
}

impl ConstMove {
    pub fn new(x: i8, y: i8, hid: Arc<RwLock<HID>>) -> Function {
        Some(Box::new(ConstMove{amount: (x, y), hid}))
    }
}

impl FunctionInterface for ConstMove {
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
        return FunctionType::ConstMove{ x: self.amount.0, y: self.amount.1}
    }
}
pub struct ConstScroll {
    amount: i8,
    period: Duration,
    prev_time: Instant,
    hid: Arc<RwLock<HID>>
}

impl ConstScroll {
    pub fn new(amount: i8, period: u64, hid: Arc<RwLock<HID>>) -> Function {
        Some(Box::new(ConstScroll{amount, period: Duration::from_millis(period), hid, prev_time: Instant::now()}))
    }
}

impl FunctionInterface for ConstScroll {
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
        return FunctionType::ConstScroll{amount: self.amount, period: self.period.as_millis() as u64}
    }
}


pub struct Move {
    dir: MouseDir,
    invert: bool,
    threshold: u16,
    scale: f64,
    hid: Arc<RwLock<HID>>,
}

impl Move {
    pub fn new(dir: MouseDir, invert: bool, threshold: u16, scale: f64, hid: Arc<RwLock<HID>>) -> Function {
        Some(Box::new(Move{dir, invert, threshold, scale, hid}))
    }
}

impl FunctionInterface for Move {
    fn event(&mut self, state: u16) -> ReturnCommand {
        let Ok(mut hid) = self.hid.write() else {
            return ReturnCommand::None;
        };

        if state > self.threshold {
            let mut val = (state as f64) / (u16::MAX as f64);

            if self.invert {
                val = -val;
            }

            val *= self.scale;
            val = if val > 1.0 {
                1.0
            } else if val < -1.0 {
                -1.0
            } else {
                val
            };

            if val < 0.0 {
                val *= i8::MIN as f64;
            } else if val > 0.0 {
                val *= i8::MAX as f64;
            };

            let val = val as i8;

            hid.mouse.move_mouse(&val, &self.dir);
        }

        ReturnCommand::None
    }

    fn ftype(&self) -> FunctionType {
        FunctionType::Move{dir: self.dir.clone(), invert: self.invert, threshold: self.threshold, scale: self.scale}
    }
}


pub struct Scroll {
    period: Duration,
    invert: bool,
    threshold: u16,
    scale: f64,
    prev_time: Instant,
    hid: Arc<RwLock<HID>>,
}

impl Scroll {
    pub fn new(period: u64, invert: bool, threshold: u16, scale: f64, hid: Arc<RwLock<HID>>) -> Function {
        Some(Box::new(Scroll{period: Duration::from_millis(period), invert, threshold, scale, prev_time: Instant::now(), hid}))
    }
}

impl FunctionInterface for Scroll {
    fn event(&mut self, state: u16) -> ReturnCommand {
        let Ok(mut hid) = self.hid.write() else {
            return ReturnCommand::None;
        };

        let now = Instant::now();
        if state > self.threshold && now.duration_since(self.prev_time) > self.period {
            self.prev_time = now;
            let mut val = (state as f64) / (u16::MAX as f64);

            if self.invert {
                val = -val;
            }

            val *= self.scale;
            val = if val > 1.0 {
                1.0
            } else if val < -1.0 {
                -1.0
            } else {
                val
            };

            if val < 0.0 {
                val *= i8::MIN as f64;
            } else if val > 0.0 {
                val *= i8::MAX as f64;
            };

            let val = val as i8;

            hid.mouse.scroll_wheel(&val);
        }

        ReturnCommand::None
    }

    fn ftype(&self) -> FunctionType {
        FunctionType::Scroll{period: self.period.as_millis() as u64, invert: self.invert, threshold: self.threshold, scale: self.scale}
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
use std::{sync::{Arc}, time::{Instant, Duration}};

use async_trait::async_trait;
use tokio::sync::RwLock;
use virt_hid::mouse::{MouseDir, MouseButton};

use super::{FunctionInterface, HID, ReturnCommand, FunctionType, Function};

/// Immediate Move function, move the mouse a set amount on press
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

#[async_trait]
impl FunctionInterface for ImmediateMove {
    async fn event(&mut self, state: u16) -> super::ReturnCommand {
        if state != 0 && self.prev_state == 0 {
            let hid = self.hid.read().await;

            hid.move_mouse(self.amount.0, MouseDir::X).await;
            hid.move_mouse(self.amount.1, MouseDir::Y).await;

            hid.send_mouse();
        }

        self.prev_state = state;
        ReturnCommand::None
    }

    fn ftype(&self) -> FunctionType {
        return FunctionType::ImmediateMove{ x: self.amount.0, y: self.amount.1}
    }
}

/// Immediate Scroll function, scroll the mouse a set amount on press
pub struct ImmediateScroll {
    amount: i8,
    prev_state: u16,
    hid: Arc<RwLock<HID>>
}

impl ImmediateScroll {
    // New
    pub fn new(amount: i8, hid: Arc<RwLock<HID>>) -> Function {
        Some(Box::new(ImmediateScroll{amount, prev_state: 0, hid}))
    }
}

#[async_trait]
impl FunctionInterface for ImmediateScroll {
    async fn event(&mut self, state: u16) -> super::ReturnCommand {
        if state != 0 && self.prev_state == 0 {
            let hid = self.hid.read().await;

            hid.scroll_wheel(self.amount).await;
            hid.send_mouse();
        }

        self.prev_state = state;
        ReturnCommand::None
    }

    fn ftype(&self) -> FunctionType {
        return FunctionType::ImmediateScroll(self.amount)
    }
}

/// Const Move function, move the mouse a set amount whilst pressed
pub struct ConstMove {
    amount: (i8, i8),
    hid: Arc<RwLock<HID>>
}

impl ConstMove {
    /// New
    pub fn new(x: i8, y: i8, hid: Arc<RwLock<HID>>) -> Function {
        Some(Box::new(ConstMove{amount: (x, y), hid}))
    }
}

#[async_trait]
impl FunctionInterface for ConstMove {
    async fn event(&mut self, state: u16) -> super::ReturnCommand {
        let hid = self.hid.read().await;

        if state != 0 {
            hid.move_mouse(self.amount.0, MouseDir::X).await;
            hid.move_mouse(self.amount.1, MouseDir::Y).await;

            hid.send_mouse();
        }
        
        return super::ReturnCommand::None
    }

    fn ftype(&self) -> FunctionType {
        return FunctionType::ConstMove{ x: self.amount.0, y: self.amount.1}
    }
}

/// Const Scroll function, scroll the mouse a set amount whilst pressed
pub struct ConstScroll {
    amount: i8,
    period: Duration,
    prev_time: Instant,
    hid: Arc<RwLock<HID>>
}

impl ConstScroll {
    /// New
    pub fn new(amount: i8, period: u64, hid: Arc<RwLock<HID>>) -> Function {
        Some(Box::new(ConstScroll{amount, period: Duration::from_millis(period), hid, prev_time: Instant::now()}))
    }
}

#[async_trait]
impl FunctionInterface for ConstScroll {
    async fn event(&mut self, state: u16) -> super::ReturnCommand {

        let hid = self.hid.read().await;

        if state != 0 {
            let now = Instant::now();
            if now.duration_since(self.prev_time) > self.period {
                self.prev_time = now;

                hid.scroll_wheel(self.amount).await;
    
                hid.send_mouse();
            }
        }
        
        return super::ReturnCommand::None
    }

    fn ftype(&self) -> FunctionType {
        return FunctionType::ConstScroll{amount: self.amount, period: self.period.as_millis() as u64}
    }
}


/// Move function, move the mouse in a direction based on the state
pub struct Move {
    dir: MouseDir,
    invert: bool,
    threshold: u16,
    scale: f64,
    hid: Arc<RwLock<HID>>,
}

impl Move {
    /// New
    pub fn new(dir: MouseDir, invert: bool, threshold: u16, scale: f64, hid: Arc<RwLock<HID>>) -> Function {
        Some(Box::new(Move{dir, invert, threshold, scale, hid}))
    }
}

#[async_trait]
impl FunctionInterface for Move {
    async fn event(&mut self, state: u16) -> ReturnCommand {
        let hid = self.hid.read().await;

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

            hid.move_mouse(val, self.dir.clone()).await;
        }

        ReturnCommand::None
    }

    fn ftype(&self) -> FunctionType {
        FunctionType::Move{dir: self.dir.clone(), invert: self.invert, threshold: self.threshold, scale: self.scale}
    }
}


/// Scroll function, move the scroll in a direction based on the state
pub struct Scroll {
    period: Duration,
    invert: bool,
    threshold: u16,
    scale: f64,
    prev_time: Instant,
    hid: Arc<RwLock<HID>>,
}

impl Scroll {
    /// New
    pub fn new(period: u64, invert: bool, threshold: u16, scale: f64, hid: Arc<RwLock<HID>>) -> Function {
        Some(Box::new(Scroll{period: Duration::from_millis(period), invert, threshold, scale, prev_time: Instant::now(), hid}))
    }
}

#[async_trait]
impl FunctionInterface for Scroll {
    async fn event(&mut self, state: u16) -> ReturnCommand {
        let hid = self.hid.read().await;

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

            hid.scroll_wheel(val).await;
        }

        ReturnCommand::None
    }

    fn ftype(&self) -> FunctionType {
        FunctionType::Scroll{period: self.period.as_millis() as u64, invert: self.invert, threshold: self.threshold, scale: self.scale}
    }
}

/// Left Click function
pub struct LeftClick {
    hid: Arc<RwLock<HID>>,
    prev_state: u16,
}

impl LeftClick {
    /// New
    pub fn new(hid: Arc<RwLock<HID>>) -> Function {
        Some(Box::new(LeftClick{hid, prev_state: 0}))
    }
}

#[async_trait]
impl FunctionInterface for LeftClick {
    async fn event(&mut self, state: u16) -> super::ReturnCommand {
        let hid = self.hid.read().await;

        if state != 0 && self.prev_state == 0 {
            hid.hold_button(MouseButton::Left).await;
            hid.send_mouse();
        } else if state == 0 && self.prev_state != 0 {
            hid.release_button(MouseButton::Left).await;
            hid.send_mouse();
        }

        self.prev_state = state;

        return super::ReturnCommand::None
    }

    fn ftype(&self) -> FunctionType {
        return FunctionType::LeftClick
    }
}

/// Right click function
pub struct RightClick {
    hid: Arc<RwLock<HID>>,
    prev_state: u16,
}

impl RightClick {
    /// New
    pub fn new(hid: Arc<RwLock<HID>>) -> Function {
        Some(Box::new(RightClick{hid, prev_state: 0}))
    }
}

#[async_trait]
impl FunctionInterface for RightClick {
    async fn event(&mut self, state: u16) -> super::ReturnCommand {
        let hid = self.hid.read().await;

        if state != 0 && self.prev_state == 0 {
            hid.hold_button(MouseButton::Right).await;
            hid.send_mouse();
        } else if state == 0 && self.prev_state != 0 {
            hid.release_button(MouseButton::Right).await;
            hid.send_mouse();
        }

        self.prev_state = state;

        return super::ReturnCommand::None
    }

    fn ftype(&self) -> FunctionType {
        return FunctionType::RightClick
    }
}
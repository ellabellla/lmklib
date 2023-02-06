use std::{sync::{Arc}, time::{Instant, Duration}};

use async_trait::async_trait;
use tokio::{sync::RwLock};
use virt_hid::mouse::{MouseDir, MouseButton};

use crate::variables::{Variable, Data};

use super::{FunctionInterface, HID, ReturnCommand, FunctionType, Function, State, StateHelpers};

/// Immediate Move function, move the mouse a set amount on press
pub struct ImmediateMove {
    amount: (Variable<i8>, Variable<i8>),
    prev_state: u16,
    hid: Arc<RwLock<HID>>
}

impl ImmediateMove {
    pub fn new(x: Variable<i8>, y: Variable<i8>, hid: Arc<RwLock<HID>>) -> Function {
        Some(Box::new(ImmediateMove{amount: (x, y), prev_state: 0, hid}))
    }
}

#[async_trait]
impl FunctionInterface for ImmediateMove {
    async fn event(&mut self, state: State) -> super::ReturnCommand {
        if state.rising(self.prev_state) {
            let hid = self.hid.read().await;

            hid.move_mouse(*self.amount.0.data(), MouseDir::X).await;
            hid.move_mouse(*self.amount.1.data(), MouseDir::Y).await;

            hid.send_mouse();
        }

        self.prev_state = state;
        ReturnCommand::None
    }

    fn ftype(&self) -> FunctionType {
        return FunctionType::ImmediateMove{ x: self.amount.0.into_data(), y: self.amount.1.into_data()}
    }
}

/// Immediate Scroll function, scroll the mouse a set amount on press
pub struct ImmediateScroll {
    amount: Variable<i8>,
    prev_state: u16,
    hid: Arc<RwLock<HID>>
}

impl ImmediateScroll {
    // New
    pub fn new(amount: Variable<i8>, hid: Arc<RwLock<HID>>) -> Function {
        Some(Box::new(ImmediateScroll{amount, prev_state: 0, hid}))
    }
}

#[async_trait]
impl FunctionInterface for ImmediateScroll {
    async fn event(&mut self, state: State) -> super::ReturnCommand {
        if state.rising(self.prev_state) {
            let hid = self.hid.read().await;

            hid.scroll_wheel(*self.amount.data()).await;
            hid.send_mouse();
        }

        self.prev_state = state;
        ReturnCommand::None
    }

    fn ftype(&self) -> FunctionType {
        return FunctionType::ImmediateScroll(self.amount.into_data())
    }
}

/// Const Move function, move the mouse a set amount whilst pressed
pub struct ConstMove {
    amount: (Variable<i8>, Variable<i8>),
    hid: Arc<RwLock<HID>>
}

impl ConstMove {
    /// New
    pub fn new(x: Variable<i8>, y: Variable<i8>, hid: Arc<RwLock<HID>>) -> Function {
        Some(Box::new(ConstMove{amount: (x, y), hid}))
    }
}

#[async_trait]
impl FunctionInterface for ConstMove {
    async fn event(&mut self, state: State) -> super::ReturnCommand {
        let hid = self.hid.read().await;

        if state.high() {
            hid.move_mouse(*self.amount.0.data(), MouseDir::X).await;
            hid.move_mouse(*self.amount.1.data(), MouseDir::Y).await;

            hid.send_mouse();
        }
        
        return super::ReturnCommand::None
    }

    fn ftype(&self) -> FunctionType {
        return FunctionType::ConstMove{ x: self.amount.0.into_data(), y: self.amount.1.into_data()}
    }
}

/// Const Scroll function, scroll the mouse a set amount whilst pressed
pub struct ConstScroll {
    amount: Variable<i8>,
    period: Variable<Duration>,
    prev_time: Instant,
    hid: Arc<RwLock<HID>>
}

impl ConstScroll {
    /// New
    pub fn new(amount: Variable<i8>, period: Variable<u64>, hid: Arc<RwLock<HID>>) -> Function {
        let period: Variable<Duration> = period.map(|period| Duration::from_millis(period));
        Some(Box::new(ConstScroll{amount, period, hid, prev_time: Instant::now()}))
    }
}

#[async_trait]
impl FunctionInterface for ConstScroll {
    async fn event(&mut self, state: State) -> super::ReturnCommand {

        let hid = self.hid.read().await;

        if state.high() {
            let now = Instant::now();
            if now.duration_since(self.prev_time) > *self.period.data() {
                self.prev_time = now;

                hid.scroll_wheel(*self.amount.data()).await;
    
                hid.send_mouse();
            }
        }
        
        return super::ReturnCommand::None
    }

    fn ftype(&self) -> FunctionType {
        let period: Data<Duration> = self.period.into_data();
        return FunctionType::ConstScroll{amount: self.amount.into_data(), period: period.map(|period| period.as_millis() as u64)}
    }
}


/// Move function, move the mouse in a direction based on the state
pub struct Move {
    dir: MouseDir,
    invert: Variable<bool>,
    threshold: Variable<u16>,
    scale: Variable<f64>,
    hid: Arc<RwLock<HID>>,
}

impl Move {
    /// New
    pub fn new(dir: MouseDir, invert: Variable<bool>, threshold: Variable<u16>, scale: Variable<f64>, hid: Arc<RwLock<HID>>) -> Function {
        Some(Box::new(Move{dir, invert, threshold, scale, hid}))
    }
}

#[async_trait]
impl FunctionInterface for Move {
    async fn event(&mut self, state: State) -> ReturnCommand {
        let hid = self.hid.read().await;

        if state > *self.threshold.data() {
            let mut val = (state as f64) / (u16::MAX as f64);

            if *self.invert.data() {
                val = -val;
            }

            val *= *self.scale.data();
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
        FunctionType::Move{dir: self.dir.clone(), invert: self.invert.into_data(), threshold: self.threshold.into_data(), scale: self.scale.into_data()}
    }
}


/// Scroll function, move the scroll in a direction based on the state
pub struct Scroll {
    period: Variable<Duration>,
    invert: Variable<bool>,
    threshold: Variable<u16>,
    scale: Variable<f64>,
    prev_time: Instant,
    hid: Arc<RwLock<HID>>,
}

impl Scroll {
    /// New
    pub fn new(period: Variable<u64>, invert: Variable<bool>, threshold: Variable<u16>, scale: Variable<f64>, hid: Arc<RwLock<HID>>) -> Function {
        let period: Variable<Duration> = period.map(|period| Duration::from_millis(period));
        Some(Box::new(Scroll{period, invert, threshold, scale, prev_time: Instant::now(), hid}))
    }
}

#[async_trait]
impl FunctionInterface for Scroll {
    async fn event(&mut self, state: State) -> ReturnCommand {
        let hid = self.hid.read().await;

        let now = Instant::now();
        if state > *self.threshold.data() && now.duration_since(self.prev_time) > *self.period.data() {
            self.prev_time = now;
            let mut val = (state as f64) / (u16::MAX as f64);

            if *self.invert.data() {
                val = -val;
            }

            val *= *self.scale.data();
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
        let period: Data<Duration> = self.period.into_data();
        FunctionType::Scroll{period: period.map(|period| period.as_millis() as u64), invert: self.invert.into_data(), threshold: self.threshold.into_data(), scale: self.scale.into_data()}
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
    async fn event(&mut self, state: State) -> super::ReturnCommand {
        let hid = self.hid.read().await;

        if state.rising(self.prev_state) {
            hid.hold_button(MouseButton::Left).await;
            hid.send_mouse();
        } else if state.falling(self.prev_state) {
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
    async fn event(&mut self, state: State) -> super::ReturnCommand {
        let hid = self.hid.read().await;

        if state.rising(self.prev_state) {
            hid.hold_button(MouseButton::Right).await;
            hid.send_mouse();
        } else if state.falling(self.prev_state) {
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
use std::sync::{Arc};

use async_trait::async_trait;
use tokio::sync::RwLock;

use crate::{driver::DriverManager, variables::Variable};

use super::{FunctionInterface, ReturnCommand, FunctionType, Function, State, StateHelpers};



pub struct Output {
    name: String,
    idx: Variable<usize>,
    state: Variable<u16>,
    prev_state: u16,
    driver_manager: Arc<RwLock<DriverManager>>,
}
impl Output {
    pub fn new(driver_name: String, idx: Variable<usize>, state: Variable<u16>, driver_manager: Arc<RwLock<DriverManager>>) -> Function {
        Some(Box::new(Output{name: driver_name, idx, state, prev_state: 0, driver_manager}))
    }
}

#[async_trait]
impl FunctionInterface for Output {
    async fn event(&mut self, state: State) -> ReturnCommand {
        if state.rising(self.prev_state) {
            let mut driver_manager = self.driver_manager.write().await;
            if let Some(driver) = driver_manager.get_mut(&self.name) {
                driver.set(*self.idx.data(), *self.state.data()).await;
            }
        }

        self.prev_state = state;
        ReturnCommand::None
    }
    fn ftype(&self) -> FunctionType {
        FunctionType::Output{driver_name: self.name.clone(), idx: self.idx.into_data(), state: self.state.into_data()}
    }
}

pub struct Flip {
    name: String,
    idx: Variable<usize>,
    prev_state: u16,
    driver_manager: Arc<RwLock<DriverManager>>,
}
impl Flip {
    pub fn new(driver_name: String, idx: Variable<usize>, driver_manager: Arc<RwLock<DriverManager>>) -> Function {
        Some(Box::new(Flip{name: driver_name, idx, prev_state: 0, driver_manager}))
    }
}

#[async_trait]
impl FunctionInterface for Flip {
    async fn event(&mut self, state: State) -> ReturnCommand {
        if state.rising(self.prev_state) {
            let mut driver_manager = self.driver_manager.write().await;
            if let Some(driver) = driver_manager.get_mut(&self.name) {
                let mut state = driver.poll(*self.idx.data());
                
                if state == 0 {
                    state = 1;
                } else {
                    state = 0;
                }                
                
                driver.set(*self.idx.data(), state).await;
            }
        }

        self.prev_state = state;
        ReturnCommand::None
    }
    fn ftype(&self) -> FunctionType {
        FunctionType::Flip{driver_name: self.name.clone(), idx: self.idx.into_data()}
    }
}
use async_trait::async_trait;
use log::{warn, info, error};
use serde::{Serialize, Deserialize};

use crate::layout::{Variable, Data};

use super::{Function, FunctionInterface, ReturnCommand, FunctionType, State, StateHelpers};

#[derive(Debug, Clone, Serialize, Deserialize)]
/// Log level
pub enum LogLevel {
    Warn,
    Info,
    Error,
}

/// Log function, logs a message
pub struct Log {
    log_level: Variable<LogLevel>,
    msg: Variable<String>,
    prev_state: u16,
}

impl Log {
    /// New
    pub fn new(log_level: Variable<LogLevel>, msg: Variable<String>) -> Function {
        Some(Box::new(Log{log_level, msg, prev_state: 0}))
    }
}

#[async_trait]
impl FunctionInterface for Log {
    async fn event(&mut self, state: State) -> ReturnCommand {
        
        if state.rising(self.prev_state) {
            let mut lock = self.log_level.write_lock_variables().await;
            match **self.log_level.data(&mut lock) {
                LogLevel::Warn => warn!("{}", self.msg.data(&mut lock)),
                LogLevel::Info => info!("{}", self.msg.data(&mut lock)),
                LogLevel::Error => error!("{}", self.msg.data(&mut lock)),
            };
        }

        self.prev_state = state;
        ReturnCommand::None
    }

    fn ftype(&self) -> FunctionType {
        let log_level: Data<LogLevel> = self.log_level.into_data();
        FunctionType::Log(log_level, self.msg.into_data())
    }
}
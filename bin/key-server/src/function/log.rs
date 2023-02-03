use async_trait::async_trait;
use log::{warn, info, error};
use serde::{Serialize, Deserialize};

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
    log_level: LogLevel,
    msg: String,
    prev_state: u16,
}

impl Log {
    /// New
    pub fn new(log_level: LogLevel, msg: String) -> Function {
        Some(Box::new(Log{log_level, msg, prev_state: 0}))
    }
}

#[async_trait]
impl FunctionInterface for Log {
    async fn event(&mut self, state: State) -> ReturnCommand {
        if state.rising(self.prev_state) {
            match self.log_level {
                LogLevel::Warn => warn!("{}", self.msg),
                LogLevel::Info => info!("{}", self.msg),
                LogLevel::Error => error!("{}", self.msg),
            };
        }

        self.prev_state = state;
        ReturnCommand::None
    }

    fn ftype(&self) -> FunctionType {
        FunctionType::Log(self.log_level.clone(), self.msg.clone())
    }
}
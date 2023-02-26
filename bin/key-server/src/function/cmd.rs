use std::{process::{Command, Child}, sync::Arc, io, thread, time::Duration};

use async_trait::async_trait;
use tokio::{sync::RwLock};

use crate::{OrLog, variables::Variable, frontend::{FrontendConfiguration, FrontendConfigData, FrontendConfig}};

use super::{Function, FunctionInterface, ReturnCommand, FunctionType, State, StateHelpers};

/// Command Pool, reaps spawn children
pub struct CommandPool {
    commands: Arc<RwLock<Vec<Child>>>,
}

#[async_trait]
impl FrontendConfig for CommandPool {
    type Output = Arc<RwLock<CommandPool>>;
    type Error = io::Error;

    fn to_config_data(&self) -> FrontendConfigData {
        FrontendConfigData::CommandPool
    }

    async fn from_config(_function_config: &FrontendConfiguration) -> Result<Self::Output, Self::Error> {
        CommandPool::new()
    }
}

impl CommandPool {
    // New
    pub fn new() -> io::Result<Arc<RwLock<CommandPool>>> {
        let commands = Arc::new(RwLock::new(Vec::<Child>::new()));

        let comms = commands.clone();
        tokio::task::spawn_blocking(move || {
            loop {
                {
                    let mut commands = comms.blocking_write();
                    let mut i = 0;
                    while i < commands.len() {
                        if let Ok(Some(_)) = commands[i].try_wait() {
                            commands.remove(i);
                        } else {
                            i+=1;
                        }
                    }
                    drop(commands);
                }

                thread::sleep(Duration::from_millis(100));
            }
        });

        Ok(Arc::new(RwLock::new(CommandPool{commands})))
    }

    /// Add command to pool
    pub async fn add_command(&mut self, command: Child) {
        self.commands.write().await.push(command);
    }
}

/// Bash Function, runs bash command
pub struct Bash {
    command: Variable<String>,
    prev_state: u16,
    command_pool: Arc<RwLock<CommandPool>>,
}

impl Bash {
    /// New
    pub fn new(command: Variable<String>, command_pool: Arc<RwLock<CommandPool>>) -> Function {
        Some(Box::new(Bash{command, prev_state: 0, command_pool}))
    }
}

#[async_trait]
impl FunctionInterface for Bash {
    async fn event(&mut self, state: State) -> ReturnCommand {
        if state.rising(self.prev_state) {
            exec(self.command.data(), &self.command_pool).await;
        }

        self.prev_state = state;
        ReturnCommand::None
    }

    fn ftype(&self) -> FunctionType {
        FunctionType::Bash(self.command.into_data())
    }
}

/// Pipe Function, pipes bash command into kout
pub struct Pipe {
    command: Variable<String>,
    prev_state: u16,
    command_pool: Arc<RwLock<CommandPool>>,
}

impl Pipe {
    /// New
    pub fn new(command: Variable<String>, command_pool: Arc<RwLock<CommandPool>>) -> Function {
        Some(Box::new(Pipe{command, prev_state: 0, command_pool}))
    }
}

#[async_trait]
impl FunctionInterface for Pipe {
    async fn event(&mut self, state: State) -> ReturnCommand {
        if state.rising(self.prev_state) {
            pipe(self.command.data(), &self.command_pool).await;
        }

        self.prev_state = state;
        ReturnCommand::None
    }

    fn ftype(&self) -> FunctionType {
        FunctionType::Pipe(self.command.into_data())
    }
}

/// Exec bash command
pub async fn exec(command: &str, command_pool: &Arc<RwLock<CommandPool>>) {
    if let Some(child) = Command::new("bash")
        .arg("-c")
        .arg(command)
        .spawn()
        .or_log("Command error (Command Pool)") {
            command_pool.write().await.add_command(child).await
        }
}

/// Exec bash command and pipe into kout (command will be formatted "{} | kout")
pub async fn pipe(command: &str, command_pool: &Arc<RwLock<CommandPool>>) {
    if let Some(child) = Command::new("bash")
        .arg("-c")
        .arg(format!("{} | kout", command))
        .spawn()
        .or_log("Command error (Command Pool)") {
            command_pool.write().await.add_command(child).await
        }
}
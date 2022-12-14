use std::{process::{Command, Child}, sync::Arc, io, thread, time::Duration};

use configfs::async_trait;
use tokio::{sync::RwLock};

use super::{Function, FunctionInterface, ReturnCommand, FunctionType};

pub struct CommandPool {
    commands: Arc<RwLock<Vec<Child>>>,
}

impl CommandPool {
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

    pub async fn add_command(&mut self, command: Child) {
        self.commands.write().await.push(command);
    }
}
pub struct Bash {
    command: String,
    prev_state: u16,
    command_pool: Arc<RwLock<CommandPool>>,
}

impl Bash {
    pub fn new(command: String, command_pool: Arc<RwLock<CommandPool>>) -> Function {
        Some(Box::new(Bash{command, prev_state: 0, command_pool}))
    }
}

#[async_trait]
impl FunctionInterface for Bash {
    async fn event(&mut self, state: u16) -> ReturnCommand {
        if state != 0 && self.prev_state == 0 {
            exec(&self.command, &self.command_pool).await;
        }

        self.prev_state = state;
        ReturnCommand::None
    }

    fn ftype(&self) -> FunctionType {
        FunctionType::Bash(self.command.clone())
    }
}

pub struct Pipe {
    command: String,
    prev_state: u16,
    command_pool: Arc<RwLock<CommandPool>>,
}

impl Pipe {
    pub fn new(command: String, command_pool: Arc<RwLock<CommandPool>>) -> Function {
        Some(Box::new(Pipe{command, prev_state: 0, command_pool}))
    }
}

#[async_trait]
impl FunctionInterface for Pipe {
    async fn event(&mut self, state: u16) -> ReturnCommand {
        if state != 0 && self.prev_state == 0 {
            pipe(&self.command, &self.command_pool).await;
        }

        self.prev_state = state;
        ReturnCommand::None
    }

    fn ftype(&self) -> FunctionType {
        FunctionType::Pipe(self.command.clone())
    }
}

pub async fn exec(command: &str, command_pool: &Arc<RwLock<CommandPool>>) {
    if let Some(child) = Command::new("bash")
        .arg("-c")
        .arg(command)
        .spawn()
        .ok() {
            command_pool.write().await.add_command(child).await
        }
}

pub async fn pipe(command: &str, command_pool: &Arc<RwLock<CommandPool>>) {
    if let Some(child) = Command::new("bash")
        .arg("-c")
        .arg(format!("{} | kout", command))
        .spawn()
        .ok() {
            command_pool.write().await.add_command(child).await
        }
}
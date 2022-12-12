use std::{process::{Command, Child}, sync::Arc, io, thread, time::Duration};

use tokio::{sync::RwLock, task::JoinHandle};

use super::{Function, FunctionInterface, ReturnCommand, FunctionType};

pub struct CommandPool {
    commands: Arc<RwLock<Vec<Child>>>
}

impl CommandPool {
    pub fn new() -> io::Result<(CommandPool, JoinHandle<()>)> {
        let commands = Arc::new(RwLock::new(Vec::<Child>::new()));

        let comms = commands.clone();
        let join = tokio::spawn(async move {
            loop {
                {
                    let mut commands = comms.write().await;
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
        Ok((CommandPool{commands}, join))
    }

    pub fn add_command(&mut self, command: Child) {
        self.commands.blocking_write().push(command);
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

impl FunctionInterface for Bash {
    fn event(&mut self, state: u16) -> ReturnCommand {
        if state != 0 && self.prev_state == 0 {
            exec(&self.command, &self.command_pool);
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

impl FunctionInterface for Pipe {
    fn event(&mut self, state: u16) -> ReturnCommand {
        if state != 0 && self.prev_state == 0 {
            pipe(&self.command, &self.command_pool);
        }

        self.prev_state = state;
        ReturnCommand::None
    }

    fn ftype(&self) -> FunctionType {
        FunctionType::Pipe(self.command.clone())
    }
}

pub fn exec(command: &str, command_pool: &Arc<RwLock<CommandPool>>) {
    Command::new("bash")
        .arg("-c")
        .arg(command)
        .spawn()
        .ok()
        .map(|child| command_pool.blocking_write().add_command(child));
}

pub fn pipe(command: &str, command_pool: &Arc<RwLock<CommandPool>>) {
        Command::new("bash")
        .arg("-c")
        .arg(format!("{} | kout", command))
        .spawn()
        .ok()
        .map(|child| command_pool.blocking_write().add_command(child));
}
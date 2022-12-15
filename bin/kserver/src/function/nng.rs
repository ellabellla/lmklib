use std::{sync::Arc, io::Write};

use configfs::async_trait;
use nanomsg::{Socket, Protocol, Error};
use serde::{Serialize, Deserialize};
use tokio::sync::RwLock;

use crate::{driver::DriverManager, OrLogIgnore, OrLog};

use super::{Function, FunctionInterface, ReturnCommand, FunctionType};


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriverData {
    name: String,    
    idx: Vec<usize>,
}


pub struct NanoMsg {
    address: String,
    msg: String,
    driver_data: Vec<DriverData>,
    prev_state: u16,
    driver_manager: Arc<RwLock<DriverManager>>,
}

impl NanoMsg {
    pub fn new(address: String, msg: String, driver_data: Vec<DriverData>, driver_manager: Arc<RwLock<DriverManager>>) -> Function {
        Some(Box::new(NanoMsg{address, msg, driver_data, prev_state: 0, driver_manager}))
    }

    async fn send(&self, data: Vec<u16>) {
        let address = self.address.clone();
        let msg = self.msg.clone();
        tokio::task::spawn_blocking(move || {
            (move || -> Result<(), Error> {
                let mut socket = Socket::new(Protocol::Pair)?;
                let mut endpoint = socket.connect(&address)?;
                
                socket.set_send_timeout(10)?;
                if data.len() == 0 {
                    socket.write_all(&format!("{}", msg).as_bytes())?;
                } else {
                    socket.write_all(&format!("{}: {:?}", msg, data).as_bytes())?;
                }
                socket.flush()?;
            

                endpoint.shutdown()?;

                Ok(())
            })().or_log("Send NanoMsg error (NanoMsg Function)");
        });
    }
}

#[async_trait]
impl FunctionInterface for NanoMsg {
    async fn event(&mut self, state: u16) -> ReturnCommand {
        if state != 0 && self.prev_state == 0 {
            let mut pin_data = Vec::with_capacity(self.driver_data.len());
            
            for driver_data in &self.driver_data {
                if let Some(driver) = self.driver_manager.read().await
                    .get(&driver_data.name)
                    .or_log_ignore(&format!("Unable to find driver {} (NanoMsg Function)", driver_data.name)) {
                        for idx in &driver_data.idx {
                            pin_data.push(driver.poll(*idx));
                        }
                }
            }

            self.send(pin_data).await;
        }

        self.prev_state = state;
        ReturnCommand::None
    }

    fn ftype(&self) -> FunctionType {
        FunctionType::NanoMsg{address: self.address.clone(), msg: self.msg.clone(), driver_data: self.driver_data.clone() }
    }
}
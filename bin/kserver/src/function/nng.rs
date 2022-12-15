use std::{sync::Arc, io::{Write}, fmt::Display};

use configfs::async_trait;
use nanomsg::{Socket, Protocol};
use serde::{Serialize, Deserialize};
use tokio::sync::{RwLock, mpsc::{UnboundedSender, self}, oneshot};

use crate::{driver::DriverManager, OrLogIgnore, OrLog};

use super::{Function, FunctionInterface, ReturnCommand, FunctionType, FunctionConfig, FunctionConfigData};

#[derive(Debug)]
pub enum NanoMsgError {
    Controller(nanomsg::Error),
    NoConfig,
    ChannelError,
}

impl Display for NanoMsgError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NanoMsgError::Controller(e) => f.write_fmt(format_args!("Nano Msg error, {}", e)),
            NanoMsgError::NoConfig => f.write_str("No configuration was supplied"),
            NanoMsgError::ChannelError => f.write_str("Channel Error"),
        }
    }
}


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriverData {
    name: String,    
    idx: Vec<usize>,
}

pub struct NanoMessenger{
    tx: UnboundedSender<Vec<u8>>,
    addresses: Vec<String>, 
    timeout: isize,
}

#[async_trait]
impl FunctionConfig for NanoMessenger {
    type Output = Arc<RwLock<NanoMessenger>>;

    type Error = NanoMsgError;

    fn to_config_data(&self) -> super::FunctionConfigData {
        FunctionConfigData::NanoMsg{addresses: self.addresses.clone(), timeout: self.timeout as i64}
    }

    async fn from_config(function_config: &super::FunctionConfiguration) -> Result<Self::Output, Self::Error> {
        let Some(FunctionConfigData::NanoMsg{addresses, timeout}) = function_config
            .get(|config| matches!(config, FunctionConfigData::NanoMsg{addresses:_, timeout:_})) else {
                return Err(NanoMsgError::NoConfig)
        };
        NanoMessenger::new(addresses.clone(), timeout.clone() as isize).await
    }
}

impl NanoMessenger {
    pub async fn new(addresses: Vec<String>, timeout: isize) -> Result<Arc<RwLock<NanoMessenger>>, NanoMsgError> {
        let (tx, mut rx) = mpsc::unbounded_channel::<Vec<u8>>();
        let (new_tx, new_rx) = oneshot::channel::<Result<(), nanomsg::Error>>();

        let add = addresses.clone();
        let time = timeout.clone();
        tokio::task::spawn_blocking(move || {
            let mut socket = match Socket::new(Protocol::Pair) {
                Ok(socket) => socket,
                Err(e) => {new_tx.send(Err(e)).or_log_ignore("Channel error (Nano Messenger"); return;}
            };
            let mut endpoints = Vec::with_capacity(addresses.len());
            for address in addresses.iter() {
                endpoints.push(match socket.connect(address) {
                    Ok(socket) => socket,
                    Err(e) => {new_tx.send(Err(e)).or_log_ignore("Channel error (Nano Messenger"); return;}
                })
            }
            new_tx.send(Ok(())).or_log_ignore("Channel error (Nano Messenger");

            while let Some(bytes) = rx.blocking_recv() {
                socket.set_send_timeout(timeout).or_log("Send NanoMsg error (NanoMsg Function)");
                socket.write_all(&bytes).or_log("Send NanoMsg error (NanoMsg Function)");
                socket.flush().or_log("Send NanoMsg error (NanoMsg Function)");            
            }

            for mut endpoint in endpoints {
                endpoint.shutdown().or_log("Send NanoMsg error (NanoMsg Function)");
            }
        });

        if let Ok(res) = new_rx.await {
            res.map(|_| Arc::new(RwLock::new(NanoMessenger{tx, addresses: add, timeout: time})))
                .map_err(|e| NanoMsgError::Controller(e))
        } else {
            Err(NanoMsgError::ChannelError)
        }
    }

    pub fn send(&self, bytes: Vec<u8>) {
        self.tx.send(bytes).or_log("Channel error (Nano Messenger)");
    }
}


pub struct NanoMsg {
    msg: String,
    driver_data: Vec<DriverData>,
    prev_state: u16,
    nano_messenger: Arc<RwLock<NanoMessenger>>,
    driver_manager: Arc<RwLock<DriverManager>>,
}

impl NanoMsg {
    pub fn new(msg: String, driver_data: Vec<DriverData>, nano_messenger: Arc<RwLock<NanoMessenger>>, driver_manager: Arc<RwLock<DriverManager>>) -> Function {
        Some(Box::new(NanoMsg{msg, driver_data, prev_state: 0, nano_messenger, driver_manager}))
    }
}

#[async_trait]
impl FunctionInterface for NanoMsg {
    async fn event(&mut self, state: u16) -> ReturnCommand {
        if state != 0 && self.prev_state == 0 {
            let mut data = Vec::with_capacity(self.driver_data.len());
            
            for driver_data in &self.driver_data {
                if let Some(driver) = self.driver_manager.read().await
                    .get(&driver_data.name)
                    .or_log_ignore(&format!("Unable to find driver {} (NanoMsg Function)", driver_data.name)) {
                        for idx in &driver_data.idx {
                            data.push(driver.poll(*idx));
                        }
                }
            }
            let data = if data.len() == 0 {
                format!("{}", self.msg).as_bytes().to_vec()
            } else {
                format!("{}: {:?}", self.msg, data).as_bytes().to_vec()
            };
            self.nano_messenger.read().await.send(data);
        }

        self.prev_state = state;
        ReturnCommand::None
    }

    fn ftype(&self) -> FunctionType {
        FunctionType::NanoMsg{msg: self.msg.clone(), driver_data: self.driver_data.clone() }
    }
}
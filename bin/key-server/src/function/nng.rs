use std::{sync::Arc, io::{Write}, fmt::Display, thread, time::Duration, str::MatchIndices};

use async_trait::async_trait;
use dynfmt::{Format, ArgumentSpec};
use nanomsg::{Socket, Protocol, Endpoint};
use serde::{Serialize, Deserialize};
use tokio::sync::{RwLock, mpsc::{UnboundedSender, self}, oneshot};

use crate::{driver::DriverManager, OrLogIgnore, OrLog, frontend::{FrontendConfig, FrontendConfigData, FrontendConfiguration}};

use super::{Function, FunctionInterface, ReturnCommand, FunctionType, State, StateHelpers};

/// Dynamic hash formatter, format("# bees", 10) = "10 bees"
struct HashFormat;

impl<'f> Format<'f> for HashFormat {
    type Iter = HashIter<'f>;

    fn iter_args(&self, format: &'f str) -> Result<Self::Iter, dynfmt::Error<'f>> {
        Ok(HashIter(format.match_indices('#')))
    }
}

struct HashIter<'f>(MatchIndices<'f, char>);

impl<'f> Iterator for HashIter<'f> {
    type Item = Result<ArgumentSpec<'f>, dynfmt::Error<'f>>;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next().map(|(index, _)| Ok(ArgumentSpec::new(index, index + 1)))
    }
}

#[derive(Debug)]
/// NanoMsg Error
pub enum NanoMsgError {
    /// Controller error
    Controller(nanomsg::Error),
    /// No configuration found
    NoConfig,
    /// Message passing error
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
/// Driver data to fetch for message
pub struct DriverData {
    name: String,    
    idx: Vec<usize>,
}

/// Nano message controller 
pub struct NanoMessenger{
    tx: UnboundedSender<Vec<u8>>,
    pub_addr: String, 
    sub_addr: String, 
    timeout: isize,
}

#[async_trait]
impl FrontendConfig for NanoMessenger {
    type Output = Arc<RwLock<NanoMessenger>>;

    type Error = NanoMsgError;

    fn to_config_data(&self) -> FrontendConfigData {
        FrontendConfigData::NanoMsg{pub_addr: self.pub_addr.clone(), sub_addr: self.sub_addr.clone(), timeout: self.timeout as i64}
    }

    async fn from_config(function_config: &FrontendConfiguration) -> Result<Self::Output, Self::Error> {
        let Some(FrontendConfigData::NanoMsg{pub_addr, sub_addr, timeout}) = function_config
            .get(|config| matches!(config, FrontendConfigData::NanoMsg{pub_addr:_, sub_addr:_, timeout:_})) else {
                return Err(NanoMsgError::NoConfig)
        };
        NanoMessenger::new(pub_addr.clone(), sub_addr.clone(), timeout.clone() as isize).await
    }
}

impl NanoMessenger {
    /// Create a device connection
    fn device_connection(addr: &str, protocol: Protocol) -> nanomsg::Result<(Socket, Endpoint)> {
        let mut socket = Socket::new_for_device(protocol)?;
        if matches!(protocol, Protocol::Sub) {
            socket.subscribe(&vec![])?;
        }
        let endpoint = socket.bind(addr)?;
        Ok((socket, endpoint))
    }

    /// Create a client connection
    fn connect(addr: &str, protocol: Protocol) -> nanomsg::Result<(Socket, Endpoint)> {
        let mut socket = Socket::new(protocol)?;
        let endpoint = socket.connect(addr)?;
        Ok((socket, endpoint))
    }

    /// New
    pub async fn new(pub_addr: String, sub_addr: String, timeout: isize) -> Result<Arc<RwLock<NanoMessenger>>, NanoMsgError> {
        let (tx, mut rx) = mpsc::unbounded_channel::<Vec<u8>>();
        let (new_tx, new_rx) = oneshot::channel::<Result<(), nanomsg::Error>>();

        let paddr = pub_addr.clone();
        let saddr = sub_addr.clone();
        let time = timeout.clone();
        tokio::task::spawn_blocking(move || {
            let (pub_soc, mut pub_end) = match NanoMessenger::device_connection(&pub_addr, Protocol::Sub) {
                Ok(socket) => socket,
                Err(e) => {new_tx.send(Err(e)).or_log_ignore("Channel error (Nano Messenger)"); return;}
            };
            let (sub_soc, mut sub_end) = match NanoMessenger::device_connection(&sub_addr, Protocol::Pub) {
                Ok(socket) => socket,
                Err(e) => {new_tx.send(Err(e)).or_log_ignore("Channel error (Nano Messenger)"); return;}
            }; 

            let (device_tx, mut device_rx) = oneshot::channel();
            let device = tokio::task::spawn_blocking(move || {
                if let Err(e) = Socket::device(&pub_soc, &sub_soc) {
                    device_tx.send(e).or_log_ignore("Channel error (Nano Messenger)");
                }
            });

            thread::sleep(Duration::from_millis(10));

            if let Ok(error) = device_rx.try_recv() {
                new_tx.send(Err(error)).or_log_ignore("Channel error (Nano Messenger)"); 
                return;
            }

            let (mut socket, mut endpoint) = match NanoMessenger::connect(&pub_addr, Protocol::Pub) {
                Ok(socket) => socket,
                Err(e) => {new_tx.send(Err(e)).or_log_ignore("Channel error (Nano Messenger)"); return;}
            };

            new_tx.send(Ok(())).or_log_ignore("Channel error (Nano Messenger)");

            while let Some(bytes) = rx.blocking_recv() {
                socket.set_send_timeout(timeout).or_log("Send NanoMsg error (NanoMsg Function)");
                socket.write_all(&bytes).or_log("Send NanoMsg error (NanoMsg Function)");
                socket.flush().or_log("Send NanoMsg error (NanoMsg Function)");            
            }

            device.abort();
            sub_end.shutdown().or_log("Send NanoMsg error (NanoMsg Function)");
            pub_end.shutdown().or_log("Send NanoMsg error (NanoMsg Function)");
            endpoint.shutdown().or_log("Send NanoMsg error (NanoMsg Function)");
        });

        if let Ok(res) = new_rx.await {
            res.map(|_| Arc::new(RwLock::new(NanoMessenger{tx, pub_addr: paddr, sub_addr: saddr, timeout: time})))
                .map_err(|e| NanoMsgError::Controller(e))
        } else {
            Err(NanoMsgError::ChannelError)
        }
    }

    /// Send message
    pub fn send(&self, bytes: Vec<u8>) {
        self.tx.send(bytes).or_log("Channel error (Nano Messenger)");
    }
}

/// NanoMsg function
pub struct NanoMsg {
    topic: u8,
    format: String,
    driver_data: Vec<DriverData>,
    prev_state: u16,
    nano_messenger: Arc<RwLock<NanoMessenger>>,
    driver_manager: Arc<RwLock<DriverManager>>,
}

impl NanoMsg {
    /// New
    pub fn new(topic: u8, format: String, driver_data: Vec<DriverData>, nano_messenger: Arc<RwLock<NanoMessenger>>, driver_manager: Arc<RwLock<DriverManager>>) -> Function {
        Some(Box::new(NanoMsg{topic, format, driver_data, prev_state: 0, nano_messenger, driver_manager}))
    }
}

#[async_trait]
impl FunctionInterface for NanoMsg {
    async fn event(&mut self, state: State) -> ReturnCommand {
        if state.rising(self.prev_state) {
            let mut data = Vec::with_capacity(self.driver_data.len());
            
            for driver_data in &self.driver_data {
                if let Some(driver) = self.driver_manager.read().await
                    .get(&driver_data.name)
                    .or_log_ignore(&format!("Unable to find driver {} (NanoMsg Function)", driver_data.name)) {
                        let mut states = Vec::with_capacity(driver_data.idx.len());
                        for idx in &driver_data.idx {
                            states.push(driver.poll(*idx));
                        }
                        data.push(states)
                }
            }
            let data = HashFormat.format(&self.format, data)
                .or_log("Formatting error (NanoMsg Function)");
            if let Some(data) =  data {
                self.nano_messenger.read().await.send(vec![self.topic].into_iter().chain(data.as_bytes().to_vec()).collect());
            } else {
                data.or_log_ignore("Formatting error (NanoMsg Function)");
            }
        }

        self.prev_state = state;
        ReturnCommand::None
    }

    fn ftype(&self) -> FunctionType {
        FunctionType::NanoMsg{topic: self.topic.clone(), format: self.format.clone(), driver_data: self.driver_data.clone() }
    }
}
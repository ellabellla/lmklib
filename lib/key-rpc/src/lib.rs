use std::{io::{Write, Read}, fmt::{Debug, Display}};

use nanomsg::{Socket, Endpoint, Protocol};
use serde::{Serialize, Deserialize};

#[derive(Debug)]
pub enum ClientError {
    Return(String),
    Serde(serde_json::Error),
    IO(std::io::Error),
}

impl Display for ClientError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ClientError::Return(ret) => f.write_fmt(format_args!("Unexpected return, {}", ret)),
            ClientError::Serde(e) => f.write_fmt(format_args!("Unable to serialize command, {}", e)),
            ClientError::IO(e) => f.write_fmt(format_args!("IO error, {}", e)),
        }
    }
}


#[derive(Debug, Serialize, Deserialize)]
pub enum Command {
    Layer,
    LayerIdx,
    NumLayers,
    AddLayer(String),
    RemoveLayer(usize),
    SwitchLayer(usize),
    UpLayer,
    DownLayer,
    SaveLayout,
    Variables,
    SetVariable(String, String),
    GetVariable(String),
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FallibleRet {
    pub ret: Option<String>
}


pub struct Client {
    socket: Socket,
    _endpoint: Endpoint,
}

impl Client {
    pub fn new(socket_str: &str) -> Result<Client, nanomsg::Error>{
        let mut socket = Socket::new(Protocol::Req)?;
        let _endpoint = socket.connect(socket_str)?;

        socket.set_receive_timeout(10)?;
        socket.set_send_timeout(10)?;

        Ok(Client { socket, _endpoint })
    }

    fn call_infallible(&mut self, command: Command) -> Result<String, ClientError> {
        let data = serde_json::to_string(&command).map_err(|e| ClientError::Serde(e))?;
        self.socket.write_all(&data.as_bytes()).map_err(|e| ClientError::IO(e))?;

        let mut buffer = String::new();
        self.socket.read_to_string(&mut buffer).map_err(|e| ClientError::IO(e))?;

        Ok(buffer)
    }

    fn call_no_ret(&mut self, command: Command) -> Result<(), ClientError> {
        let data = serde_json::to_string(&command).map_err(|e| ClientError::Serde(e))?;
        self.socket.write_all(&data.as_bytes()).map_err(|e| ClientError::IO(e))?;

        let mut buffer = String::new();
        self.socket.read_to_string(&mut buffer).map_err(|e| ClientError::IO(e))?;

        if buffer == "true" {
            Ok(())
        } else {
            Err(ClientError::Return(buffer))
        }
    }

    fn call_fallible(&mut self, command: Command) -> Result<String, ClientError> {
        let data = serde_json::to_string(&command).map_err(|e| ClientError::Serde(e))?;
        self.socket.write_all(&data.as_bytes()).map_err(|e| ClientError::IO(e))?;

        let mut buffer = String::new();
        self.socket.read_to_string(&mut buffer).map_err(|e| ClientError::IO(e))?;

        let ret: Option<FallibleRet> = serde_json::from_str(&buffer).ok();

        if let Some(ret) = ret {
            ret.ret.ok_or(ClientError::Return(buffer))
        } else {
            Err(ClientError::Return(buffer))
        }
    }

    pub fn layer(&mut self) -> Result<String, ClientError>{
        self.call_infallible(Command::Layer)
    }

    pub fn layer_idx(&mut self) -> Result<String, ClientError>{
        self.call_infallible(Command::LayerIdx)
    }

    pub fn num_layers(&mut self) -> Result<usize, ClientError>{
        self.call_infallible(Command::NumLayers)
            .and_then(|ret| usize::from_str_radix(&ret, 10).map_err(|e| ClientError::Return(e.to_string())))
    }

    pub fn add_layer(&mut self, layer: String) -> Result<(), ClientError>{
        self.call_no_ret(Command::AddLayer(layer))
    }

    pub fn remove_layer(&mut self, idx: usize) -> Result<(), ClientError>{
        self.call_no_ret(Command::RemoveLayer(idx))
    }

    pub fn switch_layer(&mut self, idx: usize) -> Result<(), ClientError>{
        self.call_no_ret(Command::SwitchLayer(idx))
    }

    pub fn down_layer(&mut self) -> Result<(), ClientError>{
        self.call_no_ret(Command::DownLayer)
    }

    pub fn up_layer(&mut self) -> Result<(), ClientError>{
        self.call_no_ret(Command::UpLayer)
    }

    pub fn save_layer(&mut self) -> Result<(), ClientError>{
        self.call_no_ret(Command::SaveLayout)
    }

    pub fn variables(&mut self) -> Result<String, ClientError> {
        self.call_infallible(Command::Variables)
    }

    pub fn get_variable(&mut self, name: String) -> Result<String, ClientError> {
        self.call_fallible(Command::GetVariable(name))
    }

    pub fn set_variable(&mut self, name: String, data: String) -> Result<(), ClientError> {
        self.call_no_ret(Command::SetVariable(name, data))
    }
}
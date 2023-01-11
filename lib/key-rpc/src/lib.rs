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
    AddLayer(String),
    SwitchLayer(usize),
    UpLayer,
    DownLayer,
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

    pub fn layer(&mut self) -> Result<String, ClientError>{
        let data = serde_json::to_string(&Command::Layer).map_err(|e| ClientError::Serde(e))?;
        self.socket.write_all(&data.as_bytes()).map_err(|e| ClientError::IO(e))?;

        let mut buffer = String::new();
        self.socket.read_to_string(&mut buffer).map_err(|e| ClientError::IO(e))?;

        if buffer != "" {
            Ok(buffer)
        } else {
            Err(ClientError::Return(buffer))
        }
    }

    pub fn add_layer(&mut self, layer: String) -> Result<(), ClientError>{
        let data = serde_json::to_string(&Command::AddLayer(layer)).map_err(|e| ClientError::Serde(e))?;
        self.socket.write_all(&data.as_bytes()).map_err(|e| ClientError::IO(e))?;

        let mut buffer = String::new();
        self.socket.read_to_string(&mut buffer).map_err(|e| ClientError::IO(e))?;

        if buffer == "true" {
            Ok(())
        } else {
            Err(ClientError::Return(buffer))
        }
    }

    pub fn switch_layer(&mut self, index: usize) -> Result<(), ClientError>{
        let data = serde_json::to_string(&Command::SwitchLayer(index)).map_err(|e| ClientError::Serde(e))?;
        self.socket.write_all(&data.as_bytes()).map_err(|e| ClientError::IO(e))?;

        let mut buffer = String::new();
        self.socket.read_to_string(&mut buffer).map_err(|e| ClientError::IO(e))?;

        if buffer == "true" {
            Ok(())
        } else {
            Err(ClientError::Return(buffer))
        }
    }

    pub fn down_layer(&mut self) -> Result<(), ClientError>{
        let data = serde_json::to_string(&Command::DownLayer).map_err(|e| ClientError::Serde(e))?;
        self.socket.write_all(&data.as_bytes()).map_err(|e| ClientError::IO(e))?;

        let mut buffer = String::new();
        self.socket.read_to_string(&mut buffer).map_err(|e| ClientError::IO(e))?;

        if buffer == "true" {
            Ok(())
        } else {
            Err(ClientError::Return(buffer))
        }
    }

    pub fn up_layer(&mut self) -> Result<(), ClientError>{
        let data = serde_json::to_string(&Command::UpLayer).map_err(|e| ClientError::Serde(e))?;
        self.socket.write_all(&data.as_bytes()).map_err(|e| ClientError::IO(e))?;

        let mut buffer = String::new();
        self.socket.read_to_string(&mut buffer).map_err(|e| ClientError::IO(e))?;

        if buffer == "true" {
            Ok(())
        } else {
            Err(ClientError::Return(buffer))
        }
    }
}
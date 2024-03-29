use std::{sync::Arc, thread, time::Duration, io::{Read, Write}, path::PathBuf, fs};

use itertools::Itertools;
use nanomsg::{Socket, Protocol};
use tokio::{sync::{RwLock, oneshot}, task::JoinHandle};
use key_rpc::{Command, FallibleRet};

use crate::{layout::{Layout}, OrLogIgnore, function::FunctionType, OrLog, variables::Variables};


pub struct ConfigRPC {
}

impl ConfigRPC {
    pub async fn start(front: String, back: String, layout: Arc<RwLock<Layout>>, layout_path: PathBuf, variables: Arc<RwLock<Variables>>, variables_path: PathBuf) -> Result<JoinHandle<()>, nanomsg::Error> {
        let (device_tx, mut device_rx) = oneshot::channel();
        {
            let back = back.clone();
            tokio::task::spawn_blocking(move || {
                let mut front_socket = match Socket::new_for_device(Protocol::Rep) {
                    Ok(socket) => socket,
                    Err(e) => {device_tx.send(e).or_log_ignore("Channel error (Config RPC)"); return},
                };
                let mut front_endpoint = match front_socket.bind(&front) {
                    Ok(endpoint) => endpoint,
                    Err(e) => {device_tx.send(e).or_log_ignore("Channel error (Config RPC)"); return},
                };

                let mut back_socket = match Socket::new_for_device(Protocol::Req){
                    Ok(socket) => socket,
                    Err(e) => {device_tx.send(e).or_log_ignore("Channel error (Config RPC)"); return},
                };
                let mut back_endpoint = match back_socket.bind(&back){
                    Ok(endpoint) => endpoint,
                    Err(e) => {device_tx.send(e).or_log_ignore("Channel error (Config RPC)"); return},
                };

                match Socket::device(&front_socket, &back_socket){
                    Ok(_) => (),
                    Err(e) => {device_tx.send(e).or_log_ignore("Channel error (Config RPC)"); return},
                };

                front_endpoint.shutdown().or_log_ignore("Unable to shutdown endpoint (Config RPC)");
                back_endpoint.shutdown().or_log_ignore("Unable to shutdown endpoint (Config RPC)");
            });
        }

        thread::sleep(Duration::from_millis(100));

        if let Ok(error) = device_rx.try_recv() {
            return Err(error)
        }

        let mut socket = Socket::new(Protocol::Rep)?;
        socket.connect(&back)?;
        socket.set_send_timeout(10)?;

        fn bool_to_str(bool: bool) -> &'static str{
            if bool {
                "true"
            } else {
                "false"
            }
        }

        Ok(tokio::task::spawn_blocking(move || {
            let mut buffer = String::new();
            loop {
                buffer.clear();

                let Some(_) = socket.read_to_string(&mut buffer).or_log_ignore("Socket error (Config RPC)") else {
                    continue;
                };

                let Some(command) = serde_json::from_str::<Command>(&buffer).or_log("Invalid RPC command (Config RPC)") else {
                    continue;
                };

                socket.write_all(&match command {
                    Command::Layer => layout.blocking_read()
                        .layout_string()
                        .unwrap_or("".to_string())
                        .as_bytes()
                        .to_owned(),
                    Command::GetLayer(index) => serde_json::to_string(
                            &FallibleRet{
                                ret: layout.blocking_read()
                                    .layout_string_at(index)
                            }
                        ).unwrap_or_else(|_| "".to_string())
                        .as_bytes()
                        .to_owned(),
                    Command::LayerIdx => layout.blocking_read()
                        .curr_layer()
                        .to_string()
                        .as_bytes()
                        .to_owned(),
                    Command::NumLayers => layout.blocking_read()
                        .num_layers()
                        .to_string()
                        .as_bytes()
                        .to_owned(),
                    Command::AddLayer(layer) => {
                        let Some(layer) = serde_json::from_str::<Vec<Vec<FunctionType>>>(&layer).or_log("Unable to parse layer (ConfigRPC)") else {
                            continue;
                        };

                        let mut layout_write = layout.blocking_write();
                        let index = layout_write.layer_len();
                        bool_to_str(
                            tokio::runtime::Handle::current()
                            .block_on(layout_write.add_layer(layer, index))
                            .is_ok()
                        )
                        .as_bytes()
                        .to_owned()
                    }
                    Command::RemoveLayer(idx) => {
                        bool_to_str(
                            layout.blocking_write()
                            .remove_layer(idx)
                            .is_some()
                        )
                        .as_bytes()
                        .to_owned()
                    }
                    Command::SwitchLayer(index) => bool_to_str(
                        layout.blocking_write()
                            .switch_layer(index).is_some()
                        )
                        .as_bytes()
                        .to_owned(),
                    Command::UpLayer => bool_to_str(
                            layout.blocking_write().
                            up_layer().
                            is_some()
                        )
                        .as_bytes()
                        .to_owned(),
                    Command::DownLayer => bool_to_str(
                            layout.blocking_write().
                            down_layer()
                            .is_some()
                        )
                        .as_bytes()
                        .to_owned(),
                    Command::SaveLayout => layout.blocking_read().to_json().or_log("Unable to serialize layout")
                        .and_then(|json| 
                            fs::File::create(&layout_path)
                            .or_log("Unable to open layout config")
                            .map(|file| (json, file))
                        )
                        .and_then(|(json, mut file)| 
                            file.write_all(&json.as_bytes())
                            .or_log("Unable to write to layout config")
                        )
                        .map(|_| "true".as_bytes().to_owned())
                        .unwrap_or_else(|| "false".as_bytes().to_owned()),
                    Command::Variables => serde_json::to_string(
                            &variables.blocking_read()
                            .variables()
                            .collect_vec()
                        ).unwrap_or_else(|_| "".to_string())
                        .as_bytes()
                        .to_owned(),
                    Command::SetVariable(name, value) => bool_to_str(
                            variables.blocking_read()
                            .update(&name, value)
                            .is_some()
                        )
                        .as_bytes()
                        .to_owned(),
                    Command::GetVariable(name) => serde_json::to_string(
                        &FallibleRet{
                            ret: variables.blocking_read().get(&name)
                        }
                    ).unwrap_or_else(|_| "".to_string())
                    .as_bytes()
                    .to_owned(),
                    Command::SaveVariables => variables.blocking_read().to_json().or_log("Unable to serialize layout")
                    .and_then(|json| 
                        fs::File::create(&variables_path)
                        .or_log("Unable to open variables")
                        .map(|file| (json, file))
                    )
                    .and_then(|(json, mut file)| 
                        file.write_all(&json.as_bytes())
                        .or_log("Unable to write to variables")
                    )
                    .map(|_| "true".as_bytes().to_owned())
                    .unwrap_or_else(|| "false".as_bytes().to_owned()),
                }).or_log_ignore("Socket error (Config RPC)");    
            }
        }))
    }
}
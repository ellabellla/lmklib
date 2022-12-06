use std::{time::Duration, sync::Arc, io};

use configfs::{BasicConfigHook, async_trait, Result, Configuration};
use tokio::sync::RwLock;
use virt_hid::{key::{LEDState}, HID};


pub struct LEDStateInterface {
    hid: Option<HID>,
    keyboard_id: u8,
    packet: u8,
}

impl LEDStateInterface {
    pub fn new(keyboard_id: u8) -> io::Result<Arc<RwLock<LEDStateInterface>>> {
        Ok(Arc::new(RwLock::new(LEDStateInterface{ packet: 0, hid: Some(HID::new(0, keyboard_id)?), keyboard_id})))
    }

    pub fn get_state(&self, state: LEDState) -> bool {
        state.get_state(self.packet)
    }

    pub fn into_configuration(interface: &Arc<RwLock<LEDStateInterface>>) -> Configuration {
        Configuration::Basic(interface.clone())
    }
}

#[async_trait]
impl BasicConfigHook for LEDStateInterface {
    async fn fetch(&mut self) -> Result<Vec<u8>> {
        Ok(vec![self.packet])
    }

    async fn size(&mut self) -> Result<u64> {
        Ok(1)
    }

    async fn update(&mut self, _data: Vec<u8>) -> Result<()> {
        Err(libc::EACCES.into())
    }
     
    async fn tick(&mut self) {
        let hid = self.hid.take();
        let packet = self.packet;
        let mut hid = match hid {
            Some(hid) => hid,
            None => {
                let id = self.keyboard_id;
                let Ok(hid) = HID::new(0, id) else {
                    return;
                };
                hid
            }
        };

        let Ok((packet, hid)) = tokio::task::spawn_blocking(move || {
            let packet = hid.receive_states_packet(Duration::from_millis(100))
                .map(|p| p.unwrap_or(packet))
                .unwrap_or(packet);
            (packet, hid)
        }).await else {
            return;
        };
        
        self.hid = Some(hid);
        self.packet = packet;
    }

    fn tick_interval(&self) -> Duration {
        Duration::from_millis(500)
    }
}
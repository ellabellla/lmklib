#![warn(missing_docs)]

pub use hid::HID;

#[cfg(not(feature = "debug"))]
mod hid {
    use std::{fs::{OpenOptions, File}, io::{Write, self, Read}};

    /// HID interface
    pub struct HID {
        mouse_hid: File,
        keyboard_hid: File,
    }
    
    impl HID {
        /// Create new HID interface
        pub fn new(mouse_id: u8, keyboard_id: u8) -> io::Result<HID>{
            Ok(HID {
                mouse_hid: OpenOptions::new()
                    .read(true)
                    .write(true)
                    .open(format!("/dev/hidg{}", mouse_id))?, 
                keyboard_hid: OpenOptions::new()
                    .read(true)
                    .write(true)
                    .open(format!("/dev/hidg{}", keyboard_id))? })
        }
        
        /// Receive raw LED states packet from HID interface. [crate::key::LEDStatePacket] provides an abstraction for raw state packets.
        pub fn receive_states_packet(&mut self) -> io::Result<u8>{
            let mut buf = [0;1];

            self.keyboard_hid.read_exact(&mut buf)?;
            Ok(buf[0])
        }

        /// Send raw key pack to HID interface. [crate::key::Keyboard] and [crate::key::KeyPacket] provides an abstractions for raw key packets.
        pub fn send_key_packet(&mut self, data: &[u8]) -> io::Result<usize> {
            self.keyboard_hid.write(data)
        }
    
        /// Send raw mouse packet to HID interface. [crate::mouse::Mouse] provides an abstractions for raw mouse packets.
        pub fn send_mouse_packet(&mut self, data: &[u8]) -> io::Result<usize> {
            self.mouse_hid.write(data)
        }
    }
    
}
#[cfg(feature = "debug")]
mod hid {
    use std::io;

    use crate::key::KeyPacket;

    /// HID interface
    pub struct HID {
    }
    
    impl HID {
        /// Create new HID interface
        pub fn new(_mouse_id: u8, _keyboard_id: u8) -> io::Result<HID>{
            Ok(HID {})
        }
        
        /// Receive raw LED states packet from HID interface. [crate::key::LEDStatePacket] provides an abstraction for raw state packets.
        pub fn receive_states_packet(&mut self) -> io::Result<u8>{
            Ok(0)
        }

        /// Send raw key pack to HID interface. [crate::key::Keyboard] and [crate::key::KeyPacket] provides an abstractions for raw key packets.
        pub fn send_key_packet(&mut self, data: &[u8]) -> io::Result<usize> {
            print!("SEND KEY: ");
            KeyPacket::print_data(data);
            Ok(data.len())
        }
    
        /// Send raw mouse packet to HID interface. [crate::mouse::Mouse] provides an abstractions for raw mouse packets.
        pub fn send_mouse_packet(&mut self, data: &[u8]) -> io::Result<usize> {
            print!("SEND MOUSE: ");
            KeyPacket::print_data(data);
            Ok(data.len())
        }
    }
}

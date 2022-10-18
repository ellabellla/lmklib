
pub use hid::HID;

#[cfg(not(feature = "debug"))]
mod hid {
    use std::{fs::{OpenOptions, File}, io::{Write, self, Read}};

    pub struct HID {
        mouse_hid: File,
        keyboard_hid: File,
    }
    
    impl HID {
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
        
        pub fn receive_states_packet(&mut self) -> io::Result<u8>{
            let mut buf = [0;1];

            self.keyboard_hid.read_exact(&mut buf)?;
            Ok(buf[0])
        }

        pub fn send_key_packet(&mut self, data: &[u8]) -> io::Result<usize> {
            self.keyboard_hid.write(data)
        }
    
        pub fn send_mouse_packet(&mut self, data: &[u8]) -> io::Result<usize> {
            self.mouse_hid.write(data)
        }
    }
    
}
#[cfg(feature = "debug")]
mod hid {
    use std::io;

    use crate::key::KeyPacket;

    pub struct HID {
    }
    
    impl HID {
        pub fn new(_mouse_id: u8, _keyboard_id: u8) -> io::Result<HID>{
            Ok(HID {})
        }
    
        pub fn send_key_packet(&mut self, data: &[u8]) -> io::Result<usize> {
            print!("SEND KEY: ");
            KeyPacket::print_data(data);
            Ok(data.len())
        }
    
        pub fn send_mouse_packet(&mut self, data: &[u8]) -> io::Result<usize> {
            print!("SEND MOUSE: ");
            KeyPacket::print_data(data);
            Ok(data.len())
        }
    }
}

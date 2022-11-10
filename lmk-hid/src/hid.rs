#![warn(missing_docs)]

use std::{io::{self, Read}, fs::File, time::Duration, os::unix::prelude::AsRawFd};

pub use hid::HID;
use nix::{poll::{ppoll, PollFd, PollFlags}, sys::time::TimeSpec};

fn read_timeout(file: &mut File, timeout: Duration) -> io::Result<Option<u8>> {
    let mut poll_fd = [PollFd::new(file.as_raw_fd(), PollFlags::POLLIN)];
    if ppoll(&mut poll_fd, Some(TimeSpec::from_duration(timeout)), None)? == 1 {
        if let Some(flags) = poll_fd[0].revents() {
            if flags.contains(PollFlags::POLLIN) {
                let mut buf = [0;1];
        
                if file.read(&mut buf)? == 1 {
                    return Ok(Some(buf[0]))
                }
            }
        }
    }
    Ok(None)
}

#[cfg(not(feature = "debug"))]
mod hid {
    use std::{fs::{OpenOptions, File}, io::{Write, self}, time::Duration};

    use super::read_timeout;
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
                    .open(format!("/dev/hidg{}", keyboard_id))?,
            })
        }
        
        /// Receive raw LED states packet from HID interface with a timeout. [crate::key::LEDStatePacket] provides an abstraction for raw state packets.
        pub fn receive_states_packet(&mut self, timeout: Duration) -> io::Result<Option<u8>>{
            read_timeout(&mut self.keyboard_hid, timeout)
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
    use std::{io, time::Duration, fs::File, io::{Write}, path::{Path}};

    use tempfile::NamedTempFile;

    use super::read_timeout;

    /// HID interface
    pub struct HID {
        mouse_file: NamedTempFile,
        keyboard_file: NamedTempFile,
        state_file: Option<File>,
    }
    
    impl HID {
        /// Create new HID interface
        pub fn new(_mouse_id: u8, _keyboard_id: u8) -> io::Result<HID>{
            Ok(HID {
                mouse_file: NamedTempFile::new()?,
                keyboard_file: NamedTempFile::new()?,
                state_file: None,
            })
        }

        /// Set file to read states from for debugging
        pub fn set_state_data(&mut self, path: &str) -> io::Result<()> {
            self.state_file = Some(File::open(path)?);
            Ok(())
        }

        /// Get path of temp file key packets are being written too
        pub fn get_keyboard_path(&self) -> &Path {
            self.keyboard_file.path()
        }

        /// Get path of temp file mouse packets are being written too
        pub fn get_mouse_path(&self) -> &Path {
            self.mouse_file.path()
        }
        
        /// Receive raw LED states packet from HID interface with a timeout. [crate::key::LEDStatePacket] provides an abstraction for raw state packets.
        pub fn receive_states_packet(&mut self, timeout: Duration) -> io::Result<Option<u8>>{
            if let Some(file) = &mut self.state_file {
                return read_timeout(file, timeout)
            }
            Ok(None)
        }

        /// Send raw key pack to HID interface. [crate::key::Keyboard] and [crate::key::KeyPacket] provides an abstractions for raw key packets.
        pub fn send_key_packet(&mut self, data: &[u8]) -> io::Result<usize> {
            self.keyboard_file.write(data)
        }
    
        /// Send raw mouse packet to HID interface. [crate::mouse::Mouse] provides an abstractions for raw mouse packets.
        pub fn send_mouse_packet(&mut self, data: &[u8]) -> io::Result<usize> {
            self.mouse_file.write(data)
        }
    }
}

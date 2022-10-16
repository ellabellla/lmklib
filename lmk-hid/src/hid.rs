use std::{fs::{OpenOptions, File}, io::{Write, self}};

pub struct HID {
    mouse_hid: File,
    keyboard_hid: File,
}

impl HID {
    pub fn new(mouse_id: u8, keyboard_id: u8) -> HID {
        HID {
            mouse_hid: OpenOptions::new()
                .read(true)
                .write(true)
                .open(format!("/dev/hidg{}", mouse_id)).unwrap(), 
            keyboard_hid: OpenOptions::new()
                .read(true)
                .write(true)
                .open(format!("/dev/hidg{}", keyboard_id)).unwrap() }
    }

    pub fn send_key_packet(&mut self, data: &[u8]) -> io::Result<usize> {
        self.keyboard_hid.write(data)
    }

    pub fn send_mouse_packet(&mut self, data: &[u8]) -> io::Result<usize> {
        self.mouse_hid.write(data)
    }
}
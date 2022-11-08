use std::{io::{self}};

use crate::HID;

pub enum MouseButton {
    Left,
    Right,
    Middle,
}

impl MouseButton {
    pub fn to_byte(&self) -> u8 {
        match self {
            MouseButton::Left => 0x01,
            MouseButton::Right => 0x02,
            MouseButton::Middle => 0x04,
        }
    }
}

pub enum MouseDir {
    X,
    Y
}


const MOUSE_DATA_BUT_IDX: usize = 0;
const MOUSE_DATA_X_IDX: usize = 1;
const MOUSE_DATA_Y_IDX: usize = 2;
const MOUSE_DATA_WHEL_IDX: usize = 3;

pub struct Mouse {
    data: [u8; 5],
    hold: u8,
}

impl Mouse {
    pub fn new() -> Mouse {
        Mouse{data:[0;5], hold: 0x00}
    }

    pub fn press_button(&mut self, button: &MouseButton) {
        self.data[MOUSE_DATA_BUT_IDX] |= button.to_byte();
    }

    pub fn hold_button(&mut self, button: &MouseButton) {
        self.hold |= button.to_byte();
    }

    pub fn release_button(&mut self, button: &MouseButton) {
        self.hold &= !button.to_byte();
    }

    pub fn move_mouse(&mut self, displacement: &i8, dir: &MouseDir) {
        match dir {
            MouseDir::X => self.data[MOUSE_DATA_X_IDX] = displacement.to_be_bytes()[0],
            MouseDir::Y => self.data[MOUSE_DATA_Y_IDX] = displacement.to_be_bytes()[0],
        }
    }

    pub fn scroll_wheel(&mut self, displacement: &i8) {
        self.data[MOUSE_DATA_WHEL_IDX] = displacement.to_be_bytes()[0];
    }

    pub fn send(&mut self, hid: &mut HID) -> io::Result<usize>{
        if self.hold == 0x00 {
            hid.send_mouse_packet(&self.data)
        } else {
            self.data[MOUSE_DATA_BUT_IDX] |= self.hold;
            hid.send_mouse_packet(&self.data)?;
            self.data = [0;5];
            self.data[MOUSE_DATA_BUT_IDX] = self.hold;
            let res = hid.send_mouse_packet(&self.data);
            self.data[MOUSE_DATA_BUT_IDX] = 0;
            res
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Mouse, MouseDir, MouseButton};

    #[test]
    fn test() {
        let mut mouse = Mouse::new();
        mouse.press_button(&MouseButton::Middle );
        mouse.move_mouse(&127, &MouseDir::X);
        mouse.move_mouse(&127, &MouseDir::Y);
        mouse.scroll_wheel(&127);
        for byte in mouse.data {
            println!("{:02x}", byte);
        }
    }
}
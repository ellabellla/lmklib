#![warn(missing_docs)]
use std::{io::{self}};

use crate::HID;

#[derive(Debug)]
/// Mouse Button
pub enum MouseButton {
 ///   Left
    Left,
 ///   Right
    Right,
 ///   Middle
    Middle,
}

impl MouseButton {
    /// Mouse bution to byte
    pub fn to_byte(&self) -> u8 {
        match self {
            MouseButton::Left => 0x01,
            MouseButton::Right => 0x02,
            MouseButton::Middle => 0x04,
        }
    }
}

#[derive(Debug)]
/// Mouse movement direction
pub enum MouseDir {
    /// X
    X,
    /// Y
    Y
}


const MOUSE_DATA_BUT_IDX: usize = 0;
const MOUSE_DATA_X_IDX: usize = 1;
const MOUSE_DATA_Y_IDX: usize = 2;
const MOUSE_DATA_WHEL_IDX: usize = 3;

/// Virtual Mouse
pub struct Mouse {
    data: [u8; 5],
    hold: u8,
}

impl Mouse {
    /// New
    pub fn new() -> Mouse {
        Mouse{data:[0;5], hold: 0x00}
    }

    /// Click mouse button
    pub fn press_button(&mut self, button: &MouseButton) {
        #[cfg(feature = "debug")]
        {
            println!("press {:?}", button);
        }
        self.data[MOUSE_DATA_BUT_IDX] |= button.to_byte();
    }

    /// Hold mouse button
    pub fn hold_button(&mut self, button: &MouseButton) {
        #[cfg(feature = "debug")]
        {
            println!("hold {:?}", button);
        }
        self.hold |= button.to_byte();
    }

    /// Release mouse button
    pub fn release_button(&mut self, button: &MouseButton) {
        #[cfg(feature = "debug")]
        {
            println!("release {:?}", button);
        }
        self.hold &= !button.to_byte();
    }

    /// Move mouse a relative amount in a direction
    pub fn move_mouse(&mut self, displacement: &i8, dir: &MouseDir) {
        #[cfg(feature = "debug")]
        {
            println!("move {:?} {:?}", displacement, dir);
        }
        match dir {
            MouseDir::X => self.data[MOUSE_DATA_X_IDX] = displacement.to_be_bytes()[0],
            MouseDir::Y => self.data[MOUSE_DATA_Y_IDX] = displacement.to_be_bytes()[0],
        }
    }

    /// Scroll the scroll wheel
    pub fn scroll_wheel(&mut self, displacement: &i8) {
        #[cfg(feature = "debug")]
        {
            println!("scroll {:?}", displacement);
        }
        self.data[MOUSE_DATA_WHEL_IDX] = displacement.to_be_bytes()[0];
    }

    /// Full buffered mouse events
    pub fn send(&mut self, hid: &mut HID) -> io::Result<usize>{
        if self.data == [0;5] && self.hold == 0x00 {
            return Ok(5)
        }

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
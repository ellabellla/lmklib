use std::{fs::File, io::{Write, self}};

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
}

impl Mouse {
    pub fn press_button(&mut self, button: &MouseButton) {
        self.data[MOUSE_DATA_BUT_IDX] |= button.to_byte();
    }

    pub fn release_button(&mut self, button: &MouseButton) {
        self.data[MOUSE_DATA_BUT_IDX] &= !button.to_byte();
    }

    pub fn move_mouse(&mut self, displacement: &i8, dir: &MouseDir) {
        match dir {
            MouseDir::X => self.data[MOUSE_DATA_X_IDX] = unsafe {std::mem::transmute_copy(displacement)},
            MouseDir::Y => self.data[MOUSE_DATA_Y_IDX] = unsafe {std::mem::transmute_copy(displacement)},
        }
    }

    pub fn scroll_wheel(&mut self, displacement: &i8) {
        self.data[MOUSE_DATA_WHEL_IDX] = unsafe {std::mem::transmute_copy(displacement)};
    }

    pub fn send(&mut self, hid: &mut File) -> io::Result<usize>{
        hid.write(&self.data)
    }
}

#[cfg(test)]
mod tests {
    use super::{Mouse, MouseDir, MouseButton};

    #[test]
    fn test() {
        let mut mouse = Mouse{data:[0;5]};
        mouse.press_button(&MouseButton::Middle );
        mouse.move_mouse(&127, &MouseDir::X);
        mouse.move_mouse(&127, &MouseDir::Y);
        mouse.scroll_wheel(&127);
        for byte in mouse.data {
            println!("{:02x}", byte);
        }
    }
}
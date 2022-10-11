
use std::{fs::{File}, io::{Write, self}};

const KEY_PACKET_KEY_LEN: usize = 6;
const KEY_PACKET_MOD_IDX: usize = 0;
const KEY_PACKET_KEY_IDX: usize = 2;

pub fn string_to_packets(str: &str) -> Vec<KeyPacket> {
    str.chars()
        .filter_map(|c| KeyPacket::from_char(&c, &KeyOrigin::Keyboard))
        .collect()
}

pub fn send_key_packets(packets: &Vec<KeyPacket>, hid: &mut File) -> io::Result<usize> {
    let mut size = 0;
    for packet in packets {
        size += packet.send(hid)?;
    }

    Ok(size)
}

pub struct KeyPacket {
    data: [u8; 8],
    key_count: usize,
}

impl KeyPacket {
    pub fn new() -> KeyPacket {
        KeyPacket { data: [0x00; 8], key_count: 0 }
    }

    pub fn from_list(modifiers: &[Modifier], keys: &[(char, KeyOrigin); 6]) -> KeyPacket {
        let mut data:[u8; 8] = [0; 8];
        data[KEY_PACKET_MOD_IDX] = Modifier::all_to_byte(modifiers);
        for (i, (c, key_origin)) in keys.iter().enumerate() {
            if let Some((modifier, byte)) = char_to_byte(c, key_origin) {
                data[KEY_PACKET_KEY_IDX + i] = byte;
                if let Some(modifier) = modifier {
                    data[KEY_PACKET_MOD_IDX] |= modifier.to_byte();
                }
            }
        }
        KeyPacket { data, key_count: KEY_PACKET_KEY_LEN }
    }

    pub fn from_char(c: &char, key_origin: &KeyOrigin) -> Option<KeyPacket> {
        let (modifier, byte) = char_to_byte(&c, key_origin)?;
        let data = [modifier.map(|m|m.to_byte()).unwrap_or(0x0), 0x0, byte, 0x0, 0x0, 0x0, 0x0, 0x0];

        Some(KeyPacket{data, key_count: KEY_PACKET_KEY_LEN})
    }

    pub fn push_modifier(&mut self, modifier: Modifier) {
        self.data[KEY_PACKET_MOD_IDX] |= modifier.to_byte();
    }

    pub fn push_key(&mut self, key: char, key_origin: &KeyOrigin) -> Option<u8> {
        let (modifier, byte) = char_to_byte(&key, key_origin)?;
        if self.key_count < KEY_PACKET_KEY_LEN {
            self.data[KEY_PACKET_KEY_IDX + self.key_count] = byte;
            self.key_count +=1;
            if let Some(modifier) = modifier {
                self.data[KEY_PACKET_MOD_IDX] |= modifier.to_byte();
            }
            Some(byte)
        } else {
            None
        }
    }

    pub fn send(&self, hid: &mut File) -> io::Result<usize>{
        hid.write(&self.data)
    }
}

pub enum Modifier {
    LeftControl,
    LeftShift,
    LeftAlt,
    LeftMeta,
    RightControl,
    RightShift,
    RightAlt,
    RightMeta,
}

impl Modifier {
    pub fn all_to_byte(modifiers: &[Modifier]) -> u8 {
        modifiers.iter()
            .map(|modi| modi.to_byte())
            .reduce(|accum, byte| accum | byte)
            .unwrap_or(0)
    }

    pub fn to_byte(&self) -> u8 {
        let base = 0x00000001;
        match self {
            Modifier::RightMeta => 0b00000001 << 7,
            Modifier::RightAlt => 0b00000001 << 6,
            Modifier::RightShift => 0b00000001 << 5,
            Modifier::RightControl => base << 4,
            Modifier::LeftMeta => base << 3,
            Modifier::LeftAlt => base << 2,
            Modifier::LeftShift => base << 1,
            Modifier::LeftControl => base,
        }
    }
}

//^(\d+) ([A-Z0-9]+) (Keyboard|Keypad|Misc) (.*?)$
pub enum KeyOrigin {
    Keyboard,
    Keypad,
    Misc,
}

pub enum SpecialKey {
    ReturnEnter,
    Return,
    Escape,
    Backspace,
    Tab,
    Spacebar,
    NONUSHashAndTilda,
    CapsLock,
    F1,
    F2,
    F3,
    UpArrow,
    DownArrow,
    LeftArrow,
    RightArrow,
    PageDown,
    End,
    DeleteForward,
    PageUp,
    Home,
    Insert,
    Pause,
    ScrollLock,
    PrintScreen,
    F12,
    F11,
    F10,
    F9,
    F8,
    F7,
    F6,
    F5,
    F4,
    NumLockAndClear,
    ENTER,
    Application,
    Power,
    RightGUI,
    RightAlt,
    RightShift,
    RightControl,
    LeftGUI,
    LeftAlt,
    LeftShift,
    LeftControl,
    Hexadecimal,
    Decimal,
    Octal,
    Binary,
    ClearEntry,
    Clear,
    PlusMinux,
    MemoryDivide,
    MemoryMultiply,
    MemorySubtract,
    MemoryAdd,
    MemoryClear,
    MemoryRecall,
    MemoryStore,
    Space,
    Or,
    And,
    XOR,
    CurrencySubunit,
    CurrencyUnit,
    DecimalSeparator,
    ThousandsSeparator,
    _000,
    _00,
    ExSel,
    CrSelProps,
    ClearAgain,
    Oper,
    Out,
    Separator,
    Prior,
    Cancel,
    SysReqAttention1,
    AlternateErase,
    LANG9,
    LANG8,
    LANG7,
    LANG6,
    LANG5,
    LANG4,
    LANG3,
    LANG2,
    LANG1,
    International9,
    International8,
    International7,
    International6,
    International5,
    International4,
    International3,
    International2,
    International1,
    LockingScrollLock,
    LockingNumLock,
    LockingCapsLock,
    VolumeDown,
    VolumeUp,
    Mute,
    Find,
    Paste,
    Copy,
    Cut,
    Undo,
    Again,
    Stop,
    Select,
    Menu,
    Help,
    Execute,
    F24,
    F23,
    F22,
    F21,
    F20,
    F19,
    F18,
    F17,
    F16,
    F15,
    F14,
    F13,
    NonUSSlashAndPipe,
    _DotAndDelete,
    _0AndInsert,
    _9AndPageUp,
    _8AndUpArrow,
    _7AndHome,
    _6AndRightArrow,
    _5,
    _4AndLeftArrow,
    _3AndPageDn,
    _2AndDownArrow,
    _1AndEnd,
    PadClear,
    PadBackspace,
    PadTab,
    EqualsSign,
    Comma,

}
pub fn char_to_byte(c:&char, key_origin: &KeyOrigin) -> Option<(Option<Modifier>, u8)> {
    match key_origin {
        KeyOrigin::Keyboard => match c {
            'a' => Some((None, 0x04)), // 4, Some((None, 0x04)), Keyboard, 'a', 'A'
            'A' => Some((Some(Modifier::LeftShift), 0x04)), // 4, Some((None, 0x04)), Keyboard, 'a', 'A'
            'b' => Some((None, 0x05)), // 5, Some((None, 0x05)), Keyboard, 'b', 'B'
            'B' => Some((Some(Modifier::LeftShift), 0x05)), // 5, Some((None, 0x05)), Keyboard, 'b', 'B'
            'c' => Some((None, 0x06)), // 6, Some((None, 0x06)), Keyboard, 'c', 'C'
            'C' => Some((Some(Modifier::LeftShift), 0x06)), // 6, Some((None, 0x06)), Keyboard, 'c', 'C'
            'd' => Some((None, 0x07)), // 7, Some((None, 0x07)), Keyboard, 'd', 'D'
            'D' => Some((Some(Modifier::LeftShift), 0x07)), // 7, Some((None, 0x07)), Keyboard, 'd', 'D'
            'e' => Some((None, 0x08)), // 8, Some((None, 0x08)), Keyboard, 'e', 'E'
            'E' => Some((Some(Modifier::LeftShift), 0x08)), // 8, Some((None, 0x08)), Keyboard, 'e', 'E'
            'f' => Some((None, 0x09)), // 9, Some((None, 0x09)), Keyboard, 'f', 'F'
            'F' => Some((Some(Modifier::LeftShift), 0x09)), // 9, Some((None, 0x09)), Keyboard, 'f', 'F'
            'g' => Some((None, 0x0A)), // 10, Some((None, 0x0A)), Keyboard, 'g', 'G'
            'G' => Some((Some(Modifier::LeftShift), 0x0A)), // 10, Some((None, 0x0A)), Keyboard, 'g', 'G'
            'h' => Some((None, 0x0B)), // 11, Some((None, 0x0B)), Keyboard, 'h', 'H'
            'H' => Some((Some(Modifier::LeftShift), 0x0B)), // 11, Some((None, 0x0B)), Keyboard, 'h', 'H'
            'i' => Some((None, 0x0C)), // 12, Some((None, 0x0C)), Keyboard, 'i', 'I'
            'I' => Some((Some(Modifier::LeftShift), 0x0C)), // 12, Some((None, 0x0C)), Keyboard, 'i', 'I'
            'j' => Some((None, 0x0D)), // 13, Some((None, 0x0D)), Keyboard, 'j', 'J'
            'J' => Some((Some(Modifier::LeftShift), 0x0D)), // 13, Some((None, 0x0D)), Keyboard, 'j', 'J'
            'k' => Some((None, 0x0E)), // 14, Some((None, 0x0E)), Keyboard, 'k', 'K'
            'K' => Some((Some(Modifier::LeftShift), 0x0E)), // 14, Some((None, 0x0E)), Keyboard, 'k', 'K'
            'l' => Some((None, 0x0F)), // 15, Some((None, 0x0F)), Keyboard, 'l', 'L'
            'L' => Some((Some(Modifier::LeftShift), 0x0F)), // 15, Some((None, 0x0F)), Keyboard, 'l', 'L'
            'm' => Some((None, 0x10)), // 16, Some((None, 0x10)), Keyboard, 'm', 'M'
            'M' => Some((Some(Modifier::LeftShift), 0x10)), // 16, Some((None, 0x10)), Keyboard, 'm', 'M'
            'n' => Some((None, 0x11)), // 17, Some((None, 0x11)), Keyboard, 'n', 'N'
            'N' => Some((Some(Modifier::LeftShift), 0x11)), // 17, Some((None, 0x11)), Keyboard, 'n', 'N'
            'o' => Some((None, 0x12)), // 18, Some((None, 0x12)), Keyboard, 'o', 'O'
            'O' => Some((Some(Modifier::LeftShift), 0x12)), // 18, Some((None, 0x12)), Keyboard, 'o', 'O'
            'p' => Some((None, 0x13)), // 19, Some((None, 0x13)), Keyboard, 'p', 'P'
            'P' => Some((Some(Modifier::LeftShift), 0x13)), // 19, Some((None, 0x13)), Keyboard, 'p', 'P'
            'q' => Some((None, 0x14)), // 20, Some((None, 0x14)), Keyboard, 'q', 'Q'
            'Q' => Some((Some(Modifier::LeftShift), 0x14)), // 20, Some((None, 0x14)), Keyboard, 'q', 'Q'
            'r' => Some((None, 0x15)), // 21, Some((None, 0x15)), Keyboard, 'r', 'R'
            'R' => Some((Some(Modifier::LeftShift), 0x15)), // 21, Some((None, 0x15)), Keyboard, 'r', 'R'
            's' => Some((None, 0x16)), // 22, Some((None, 0x16)), Keyboard, 's', 'S'
            'S' => Some((Some(Modifier::LeftShift), 0x16)), // 22, Some((None, 0x16)), Keyboard, 's', 'S'
            't' => Some((None, 0x17)), // 23, Some((None, 0x17)), Keyboard, 't', 'T'
            'T' => Some((Some(Modifier::LeftShift), 0x17)), // 23, Some((None, 0x17)), Keyboard, 't', 'T'
            'u' => Some((None, 0x18)), // 24, Some((None, 0x18)), Keyboard, 'u', 'U'
            'U' => Some((Some(Modifier::LeftShift), 0x18)), // 24, Some((None, 0x18)), Keyboard, 'u', 'U'
            'v' => Some((None, 0x19)), // 25, Some((None, 0x19)), Keyboard, 'v', 'V'
            'V' => Some((Some(Modifier::LeftShift), 0x19)), // 25, Some((None, 0x19)), Keyboard, 'v', 'V'
            'w' => Some((None, 0x1A)), // 26, Some((None, 0x1A)), Keyboard, 'w', 'W'
            'W' => Some((Some(Modifier::LeftShift), 0x1A)), // 26, Some((None, 0x1A)), Keyboard, 'w', 'W'
            'x' => Some((None, 0x1B)), // 27, Some((None, 0x1B)), Keyboard, 'x', 'X'
            'X' => Some((Some(Modifier::LeftShift), 0x1B)), // 27, Some((None, 0x1B)), Keyboard, 'x', 'X'
            'y' => Some((None, 0x1C)), // 28, Some((None, 0x1C)), Keyboard, 'y', 'Y'
            'Y' => Some((Some(Modifier::LeftShift), 0x1C)), // 28, Some((None, 0x1C)), Keyboard, 'y', 'Y'
            'z' => Some((None, 0x1D)), // 29, Some((None, 0x1D)), Keyboard, 'z', 'Z'
            'Z' => Some((Some(Modifier::LeftShift), 0x1D)), // 29, Some((None, 0x1D)), Keyboard, 'z', 'Z'
            '1' => Some((None, 0x1E)), // 30, Some((None, 0x1E)), Keyboard, '1', '!'
            '!' => Some((Some(Modifier::LeftShift), 0x1E)), // 30, Some((None, 0x1E)), Keyboard, '1', '!'
            '2' => Some((None, 0x1F)), // 31, Some((None, 0x1F)), Keyboard, '2', '@'
            '@' => Some((Some(Modifier::LeftShift), 0x1F)), // 31, Some((None, 0x1F)), Keyboard, '2', '@'
            '3' => Some((None, 0x20)), // 32, Some((None, 0x20)), Keyboard, '3', '#'
            '#' => Some((Some(Modifier::LeftShift), 0x20)), // 32, Some((None, 0x20)), Keyboard, '3', '#'
            '4' => Some((None, 0x21)), // 33, Some((None, 0x21)), Keyboard, '4', '$'
            '$' => Some((Some(Modifier::LeftShift), 0x21)), // 33, Some((None, 0x21)), Keyboard, '4', '$'
            '5' => Some((None, 0x22)), // 34, Some((None, 0x22)), Keyboard, '5', '%'
            '%' => Some((Some(Modifier::LeftShift), 0x22)), // 34, Some((None, 0x22)), Keyboard, '5', '%'
            '6' => Some((None, 0x23)), // 35, Some((None, 0x23)), Keyboard, '6', '^'
            '^' => Some((Some(Modifier::LeftShift), 0x23)), // 35, Some((None, 0x23)), Keyboard, '6', '^'
            '7' => Some((None, 0x24)), // 36, Some((None, 0x24)), Keyboard, '7', '&'
            '&' => Some((Some(Modifier::LeftShift), 0x24)), // 36, Some((None, 0x24)), Keyboard, '7', '&'
            '8' => Some((None, 0x25)), // 37, Some((None, 0x25)), Keyboard, '8', '*'
            '*' => Some((Some(Modifier::LeftShift), 0x25)), // 37, Some((None, 0x25)), Keyboard, '8', '*'
            '9' => Some((None, 0x26)), // 38, Some((None, 0x26)), Keyboard, '9', '('
            '(' => Some((Some(Modifier::LeftShift), 0x26)), // 38, Some((None, 0x26)), Keyboard, '9', '('
            '0' => Some((None, 0x27)), // 39, Some((None, 0x27)), Keyboard, '0', ')'
            ')' => Some((Some(Modifier::LeftShift), 0x27)), // 39, Some((None, 0x27)), Keyboard, '0', ')'
            '-' => Some((None, 0x2D)), // 45, Some((None, 0x2D)), Keyboard, '-', '_'
            '_' => Some((Some(Modifier::LeftShift), 0x2D)), // 45, Some((None, 0x2D)), Keyboard, '-', '_'
            '=' => Some((None, 0x2E)), // 46, Some((None, 0x2E)), Keyboard, '=', '+'
            '+' => Some((Some(Modifier::LeftShift), 0x2E)), // 46, Some((None, 0x2E)), Keyboard, '=', '+'
            '[' => Some((None, 0x2F)), // 47, Some((None, 0x2F)), Keyboard, '[', '{'
            '{' => Some((Some(Modifier::LeftShift), 0x2F)), // 47, Some((None, 0x2F)), Keyboard, '[', '{'
            ']' => Some((None, 0x30)), // 48, Some((None, 0x30)), Keyboard, ']', '}'
            '}' => Some((Some(Modifier::LeftShift), 0x30)), // 48, Some((None, 0x30)), Keyboard, ']', '}'
            '\\' => Some((None, 0x31)), // 49, Some((None, 0x31)), Keyboard, '\\', '|'
            '|' => Some((Some(Modifier::LeftShift), 0x31)), // 49, Some((None, 0x31)), Keyboard, '\\', '|'
            ';' => Some((None, 0x33)), // 51, Some((None, 0x33)), Keyboard, ';', ':'
            ':' => Some((Some(Modifier::LeftShift), 0x33)), // 51, Some((None, 0x33)), Keyboard, ';', ':'
            '\''  => Some((None, 0x34)), // 52, Some((None, 0x34)), Keyboard, '\'', '“'
            '“' => Some((Some(Modifier::LeftShift), 0x34)), // 52, Some((None, 0x34)), Keyboard, '\'', '“'
            '~' => Some((None, 0x35)), // 53, Some((None, 0x35)), Keyboard, '~', '`'
            '`' => Some((Some(Modifier::LeftShift), 0x35)), // 53, Some((None, 0x35)), Keyboard, '~', '`'
            ',' => Some((None, 0x36)), // 54, Some((None, 0x36)), Keyboard, ',', '<'
            '<' => Some((Some(Modifier::LeftShift), 0x36)), // 54, Some((None, 0x36)), Keyboard, ',', '<'
            '.' => Some((None, 0x37)), // 55, Some((None, 0x37)), Keyboard, '.', '>'
            '>' => Some((Some(Modifier::LeftShift), 0x37)), // 55, Some((None, 0x37)), Keyboard, '.', '>'
            '/' => Some((None, 0x38)), // 56, Some((None, 0x38)), Keyboard, '/', '?'
            '?' => Some((Some(Modifier::LeftShift), 0x38)), // 56, Some((None, 0x38)), Keyboard, '/', '?'
            _=>None,
        },
        KeyOrigin::Keypad => match c {
            '/' => Some((None, 0x54)), // 84, Some((None, 0x54)), Keypad, '/'
            '*' => Some((None, 0x55)), // 85, Some((None, 0x55)), Keypad, '*'
            '-' => Some((None, 0x56)), // 86, Some((None, 0x56)), Keypad, '-'
            '+' => Some((None, 0x57)), // 87, Some((None, 0x57)), Keypad, '+'
            '=' => Some((None, 0x67)), // 103, Some((None, 0x67)), Keypad, '='
            '(' => Some((None, 0xB6)), // 182, Some((None, 0xB6)), Keypad, '('
            ')' => Some((None, 0xB7)), // 183, Some((None, 0xB7)), Keypad, ')'
            '{' => Some((None, 0xB8)), // 184, Some((None, 0xB8)), Keypad, '{'
            '}' => Some((None, 0xB9)), // 185, Some((None, 0xB9)), Keypad, '}'
            'A' => Some((None, 0xBC)), // 188, Some((None, 0xBC)), Keypad, 'A'
            'B' => Some((None, 0xBD)), // 189, Some((None, 0xBD)), Keypad, 'B'
            'C' => Some((None, 0xBE)), // 190, Some((None, 0xBE)), Keypad, 'C'
            'D' => Some((None, 0xBF)), // 191, Some((None, 0xBF)), Keypad, 'D'
            'E' => Some((None, 0xC0)), // 192, Some((None, 0xC0)), Keypad, 'E'
            'F' => Some((None, 0xC1)), // 193, Some((None, 0xC1)), Keypad, 'F'
            '^' => Some((None, 0xC3)), // 195, Some((None, 0xC3)), Keypad, '^'
            '%' => Some((None, 0xC4)), // 196, Some((None, 0xC4)), Keypad, '%'
            '<' => Some((None, 0xC5)), // 197, Some((None, 0xC5)), Keypad, '<'
            '>' => Some((None, 0xC6)), // 198, Some((None, 0xC6)), Keypad, '>'
            '&' => Some((None, 0xC7)), // 199, Some((None, 0xC7)), Keypad, '&'
            '|' => Some((None, 0xC9)), // 201, Some((None, 0xC9)), Keypad, '|'
            ':' => Some((None, 0xCB)), // 203, Some((None, 0xCB)), Keypad, ':'
            '#' => Some((None, 0xCC)), // 204, Some((None, 0xCC)), Keypad, '#'
            '@' => Some((None, 0xCE)), // 206, Some((None, 0xCE)), Keypad, '@'
            '!' => Some((None, 0xCF)), // 207, Some((None, 0xCF)), Keypad, '!'
            _=>None,
        },
        KeyOrigin::Misc => None,
    }
}

pub fn special_to_byte(special: SpecialKey) -> u8 {
    match special {
        SpecialKey::ReturnEnter => 0x28, // 40, 0x28, Keyboard, ReturnEnter
        SpecialKey::Escape  => 0x29, // 41, 0x29, Keyboard, Escape 
        SpecialKey::Backspace => 0x2A, // 42, 0x2A, Keyboard, Backspace
        SpecialKey::Tab => 0x2B, // 43, 0x2B, Keyboard, Tab
        SpecialKey::Spacebar => 0x2C, // 44, 0x2C, Keyboard, Spacebar
        SpecialKey::NONUSHashAndTilda => 0x32, // 50, 0x32, Keyboard, NONUSHashAndTilda
        SpecialKey::CapsLock  => 0x39, // 57, 0x39, Keyboard, CapsLock 
        SpecialKey::F1  => 0x3A, // 58, 0x3A, Keyboard, F1 
        SpecialKey::F2  => 0x3B, // 59, 0x3B, Keyboard, F2 
        SpecialKey::F3  => 0x3C, // 60, 0x3C, Keyboard, F3 
        SpecialKey::F4  => 0x3D, // 61, 0x3D, Keyboard, F4 
        SpecialKey::F5  => 0x3E, // 62, 0x3E, Keyboard, F5 
        SpecialKey::F6  => 0x3F, // 63, 0x3F, Keyboard, F6 
        SpecialKey::F7  => 0x40, // 64, 0x40, Keyboard, F7 
        SpecialKey::F8  => 0x41, // 65, 0x41, Keyboard, F8 
        SpecialKey::F9  => 0x42, // 66, 0x42, Keyboard, F9 
        SpecialKey::F10  => 0x43, // 67, 0x43, Keyboard, F10 
        SpecialKey::F11  => 0x44, // 68, 0x44, Keyboard, F11 
        SpecialKey::F12  => 0x45, // 69, 0x45, Keyboard, F12 
        SpecialKey::PrintScreen  => 0x46, // 70, 0x46, Keyboard, PrintScreen 
        SpecialKey::ScrollLock  => 0x47, // 71, 0x47, Keyboard, ScrollLock 
        SpecialKey::Pause  => 0x48, // 72, 0x48, Keyboard, Pause 
        SpecialKey::Insert  => 0x49, // 73, 0x49, Keyboard, Insert 
        SpecialKey::Home  => 0x4A, // 74, 0x4A, Keyboard, Home 
        SpecialKey::PageUp  => 0x4B, // 75, 0x4B, Keyboard, PageUp 
        SpecialKey::DeleteForward => 0x4C, // 76, 0x4C, Keyboard, DeleteForward
        SpecialKey::End => 0x4D, // 77, 0x4D, Keyboard, End
        SpecialKey::PageDown => 0x4E, // 78, 0x4E, Keyboard, PageDown
        SpecialKey::RightArrow  => 0x4F, // 79, 0x4F, Keyboard, RightArrow 
        SpecialKey::LeftArrow  => 0x50, // 80, 0x50, Keyboard, LeftArrow 
        SpecialKey::DownArrow  => 0x51, // 81, 0x51, Keyboard, DownArrow 
        SpecialKey::UpArrow  => 0x52, // 82, 0x52, Keyboard, UpArrow 
        SpecialKey::NonUSSlashAndPipe => 0x64, // 100, 0x64, Keyboard, NonUSSlashAndPipe
        SpecialKey::Application  => 0x65, // 101, 0x65, Keyboard, Application 
        SpecialKey::Power => 0x66, // 102, 0x66, Keyboard, Power
        SpecialKey::F13 => 0x68, // 104, 0x68, Keyboard, F13
        SpecialKey::F14 => 0x69, // 105, 0x69, Keyboard, F14
        SpecialKey::F15 => 0x6A, // 106, 0x6A, Keyboard, F15
        SpecialKey::F16 => 0x6B, // 107, 0x6B, Keyboard, F16
        SpecialKey::F17 => 0x6C, // 108, 0x6C, Keyboard, F17
        SpecialKey::F18 => 0x6D, // 109, 0x6D, Keyboard, F18
        SpecialKey::F19 => 0x6E, // 110, 0x6E, Keyboard, F19
        SpecialKey::F20 => 0x6F, // 111, 0x6F, Keyboard, F20
        SpecialKey::F21 => 0x70, // 112, 0x70, Keyboard, F21
        SpecialKey::F22 => 0x71, // 113, 0x71, Keyboard, F22
        SpecialKey::F23 => 0x72, // 114, 0x72, Keyboard, F23
        SpecialKey::F24 => 0x73, // 115, 0x73, Keyboard, F24
        SpecialKey::Execute => 0x74, // 116, 0x74, Keyboard, Execute
        SpecialKey::Help => 0x75, // 117, 0x75, Keyboard, Help
        SpecialKey::Menu => 0x76, // 118, 0x76, Keyboard, Menu
        SpecialKey::Select => 0x77, // 119, 0x77, Keyboard, Select
        SpecialKey::Stop => 0x78, // 120, 0x78, Keyboard, Stop
        SpecialKey::Again => 0x79, // 121, 0x79, Keyboard, Again
        SpecialKey::Undo => 0x7A, // 122, 0x7A, Keyboard, Undo
        SpecialKey::Cut => 0x7B, // 123, 0x7B, Keyboard, Cut
        SpecialKey::Copy => 0x7C, // 124, 0x7C, Keyboard, Copy
        SpecialKey::Paste => 0x7D, // 125, 0x7D, Keyboard, Paste
        SpecialKey::Find => 0x7E, // 126, 0x7E, Keyboard, Find
        SpecialKey::Mute => 0x7F, // 127, 0x7F, Keyboard, Mute
        SpecialKey::VolumeUp => 0x80, // 128, 0x80, Keyboard, VolumeUp
        SpecialKey::VolumeDown => 0x81, // 129, 0x81, Keyboard, VolumeDown
        SpecialKey::LockingCapsLock => 0x82, // 130, 0x82, Keyboard, LockingCapsLock
        SpecialKey::LockingNumLock => 0x83, // 131, 0x83, Keyboard, LockingNumLock
        SpecialKey::LockingScrollLock => 0x84, // 132, 0x84, Keyboard, LockingScrollLock
        SpecialKey::International1 => 0x87, // 135, 0x87, Keyboard, International1,
        SpecialKey::International2 => 0x88, // 136, 0x88, Keyboard, International2
        SpecialKey::International3 => 0x89, // 137, 0x89, Keyboard, International3
        SpecialKey::International4 => 0x8A, // 138, 0x8A, Keyboard, International4
        SpecialKey::International5 => 0x8B, // 139, 0x8B, Keyboard, International5
        SpecialKey::International6 => 0x8C, // 140, 0x8C, Keyboard, International6
        SpecialKey::International7 => 0x8D, // 141, 0x8D, Keyboard, International7
        SpecialKey::International8 => 0x8E, // 142, 0x8E, Keyboard, International8
        SpecialKey::International9 => 0x8F, // 143, 0x8F, Keyboard, International9
        SpecialKey::LANG1 => 0x90, // 144, 0x90, Keyboard, LANG1
        SpecialKey::LANG2 => 0x91, // 145, 0x91, Keyboard, LANG2
        SpecialKey::LANG3 => 0x92, // 146, 0x92, Keyboard, LANG3
        SpecialKey::LANG4 => 0x93, // 147, 0x93, Keyboard, LANG4
        SpecialKey::LANG5 => 0x94, // 148, 0x94, Keyboard, LANG5
        SpecialKey::LANG6 => 0x95, // 149, 0x95, Keyboard, LANG6
        SpecialKey::LANG7 => 0x96, // 150, 0x96, Keyboard, LANG7
        SpecialKey::LANG8 => 0x97, // 151, 0x97, Keyboard, LANG8
        SpecialKey::LANG9 => 0x98, // 152, 0x98, Keyboard, LANG9
        SpecialKey::AlternateErase => 0x99, // 153, 0x99, Keyboard, AlternateErase
        SpecialKey::SysReqAttention1 => 0x9A, // 154, 0x9A, Keyboard, SysReqAttention1
        SpecialKey::Cancel => 0x9B, // 155, 0x9B, Keyboard, Cancel
        SpecialKey::Clear => 0x9C, // 156, 0x9C, Keyboard, Clear
        SpecialKey::Prior => 0x9D, // 157, 0x9D, Keyboard, Prior
        SpecialKey::Return => 0x9E, // 158, 0x9E, Keyboard, Return
        SpecialKey::Separator => 0x9F, // 159, 0x9F, Keyboard, Separator
        SpecialKey::Out => 0xA0, // 160, 0xA0, Keyboard, Out
        SpecialKey::Oper => 0xA1, // 161, 0xA1, Keyboard, Oper
        SpecialKey::ClearAgain => 0xA2, // 162, 0xA2, Keyboard, ClearAgain
        SpecialKey::CrSelProps => 0xA3, // 163, 0xA3, Keyboard, CrSelProps
        SpecialKey::ExSel => 0xA4, // 164, 0xA4, Keyboard, ExSel
        SpecialKey::LeftControl  => 0xE0, // 224, 0xE0, Keyboard, LeftControl 
        SpecialKey::LeftShift  => 0xE1, // 225, 0xE1, Keyboard, LeftShift 
        SpecialKey::LeftAlt  => 0xE2, // 226, 0xE2, Keyboard, LeftAlt 
        SpecialKey::LeftGUI => 0xE3, // 227, 0xE3, Keyboard, LeftGUI
        SpecialKey::RightControl  => 0xE4, // 228, 0xE4, Keyboard, RightControl 
        SpecialKey::RightShift  => 0xE5, // 229, 0xE5, Keyboard, RightShift 
        SpecialKey::RightAlt  => 0xE6, // 230, 0xE6, Keyboard, RightAlt 
        SpecialKey::RightGUI => 0xE7, // 231, 0xE7, Keyboard, RightGUI
        SpecialKey::ThousandsSeparator => 0xB2, // 178, 0xB2, Misc, ThousandsSeparator
        SpecialKey::DecimalSeparator => 0xB3, // 179, 0xB3, Misc, DecimalSeparator
        SpecialKey::CurrencyUnit => 0xB4, // 180, 0xB4, Misc, CurrencyUnit
        SpecialKey::CurrencySubunit => 0xB5, // 181, 0xB5, Misc, CurrencySubunit
        SpecialKey::NumLockAndClear  => 0x53, // 83, 0x53, Keypad, NumLockAndClear 
        SpecialKey::ENTER => 0x58, // 88, 0x58, Keypad, ENTER
        SpecialKey::_1AndEnd  => 0x59, // 89, 0x59, Keypad, _1AndEnd 
        SpecialKey::_2AndDownArrow  => 0x5A, // 90, 0x5A, Keypad, _2AndDownArrow 
        SpecialKey::_3AndPageDn  => 0x5B, // 91, 0x5B, Keypad, _3AndPageDn 
        SpecialKey::_4AndLeftArrow  => 0x5C, // 92, 0x5C, Keypad, _4AndLeftArrow 
        SpecialKey::_5 => 0x5D, // 93, 0x5D, Keypad, _5
        SpecialKey::_6AndRightArrow  => 0x5E, // 94, 0x5E, Keypad, _6AndRightArrow 
        SpecialKey::_7AndHome  => 0x5F, // 95, 0x5F, Keypad, _7AndHome 
        SpecialKey::_8AndUpArrow  => 0x60, // 96, 0x60, Keypad, _8AndUpArrow 
        SpecialKey::_9AndPageUp  => 0x61, // 97, 0x61, Keypad, _9AndPageUp 
        SpecialKey::_0AndInsert  => 0x62, // 98, 0x62, Keypad, _0AndInsert 
        SpecialKey::_DotAndDelete  => 0x63, // 99, 0x63, Keypad, _DotAndDelete 
        SpecialKey::_00 => 0xB0, // 176, 0xB0, Keypad, _00
        SpecialKey::_000 => 0xB1, // 177, 0xB1, Keypad, _000
        SpecialKey::PadTab => 0xBA, // 186, 0xBA, Keypad, Tab
        SpecialKey::PadBackspace => 0xBB, // 187, 0xBB, Keypad, Backspace
        SpecialKey::XOR => 0xC2, // 194, 0xC2, Keypad, XOR
        SpecialKey::And => 0xC8, // 200, 0xC8, Keypad, And
        SpecialKey::Or => 0xCA, // 202, 0xCA, Keypad, Or
        SpecialKey::Space => 0xCD, // 205, 0xCD, Keypad, Space
        SpecialKey::MemoryStore => 0xD0, // 208, 0xD0, Keypad, MemoryStore
        SpecialKey::MemoryRecall => 0xD1, // 209, 0xD1, Keypad, MemoryRecall
        SpecialKey::MemoryClear => 0xD2, // 210, 0xD2, Keypad, MemoryClear
        SpecialKey::MemoryAdd => 0xD3, // 211, 0xD3, Keypad, MemoryAdd
        SpecialKey::MemorySubtract => 0xD4, // 212, 0xD4, Keypad, MemorySubtract
        SpecialKey::MemoryMultiply => 0xD5, // 213, 0xD5, Keypad, MemoryMultiply
        SpecialKey::MemoryDivide => 0xD6, // 214, 0xD6, Keypad, MemoryDivide
        SpecialKey::PlusMinux => 0xD7, // 215, 0xD7, Keypad, PlusMinux
        SpecialKey::PadClear => 0xD8, // 216, 0xD8, Keypad, Clear
        SpecialKey::ClearEntry => 0xD9, // 217, 0xD9, Keypad, ClearEntry
        SpecialKey::Binary => 0xDA, // 218, 0xDA, Keypad, Binary
        SpecialKey::Octal => 0xDB, // 219, 0xDB, Keypad, Octal
        SpecialKey::Decimal => 0xDC, // 220, 0xDC, Keypad, Decimal
        SpecialKey::Hexadecimal => 0xDD, // 221, 0xDD, Keypad, Hexadecimal
        SpecialKey::Comma => 0x85, // 133, Some(0x85), Keypad, ','
        SpecialKey::EqualsSign => 0x86, // 134, Some(0x86), Keypad, '='
    }
}
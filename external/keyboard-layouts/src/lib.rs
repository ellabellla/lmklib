#![doc = include_str!("../README.md")]

use gen_layouts_sys::*;

const UNICODE_ENTER: u16 = 10; // \n
const UNICODE_TAB: u16 = 9; // \t
// https://stackoverflow.com/questions/23320417/what-is-this-character-separator
const CONTROL_CHARACTER_OFFSET: u16 = 0x40;
const UNICODE_FIRST_ASCII: u16 = 0x20; // SPACE
const UNICODE_LAST_ASCII: u16 = 0x7F; // BACKSPACE
const _UNICODE_DIGIT_OFFSET: usize = 48; // 0
const KEY_MASK: u16 = 0x3F; // Remove SHIFT/ALT/CTRL from keycode


/// Keycode
pub enum Keycode {
    ModifierKeySequence(u16, Vec<u16>),
    RegularKey(u16),
    InvalidCharacter,
}

/// Get a list of the supported keyboard layouts
pub fn available_layouts() -> Vec<&'static str> {
    LAYOUT_MAP.keys().map(|k| *k).collect()
}

/// Get the keycode for the given unicode character
pub fn keycode_for_unicode(layout: &Layout, unicode: u16) -> Keycode {
    match unicode {
        u if u == UNICODE_ENTER => Keycode::RegularKey(ENTER_KEYCODE & layout.keycode_mask),
        u if u == UNICODE_TAB => Keycode::RegularKey(TAB_KEYCODE & layout.keycode_mask),
        u if u < UNICODE_FIRST_ASCII => {
            let idx = ((u + CONTROL_CHARACTER_OFFSET) - UNICODE_FIRST_ASCII) as usize;
            let keycodes = vec![layout.keycodes[idx]];
            Keycode::ModifierKeySequence(RIGHT_CTRL_MODIFIER, keycodes)
        }
        u if u >= UNICODE_FIRST_ASCII && u <= UNICODE_LAST_ASCII => {
            let idx = (u - UNICODE_FIRST_ASCII) as usize;
            Keycode::RegularKey(layout.keycodes[idx])
        }
        _ => Keycode::InvalidCharacter,
    }
}

// https://github.com/PaulStoffregen/cores/blob/master/teensy3/usb_keyboard.c
/// Apply Deadkey mask
pub fn deadkey_for_keycode(layout: &Layout, keycode: u16) -> Option<u16> {
    layout.dead_keys_mask.and_then(|dkm| {
        let keycode = keycode & dkm;
        if let Some(acute_accent_bits) = layout.deadkeys.acute_accent_bits {
            if keycode == acute_accent_bits {
                return layout.deadkeys.deadkey_accute_accent;
            }
        }
        if let Some(cedilla_bits) = layout.deadkeys.cedilla_bits {
            if keycode == cedilla_bits {
                return layout.deadkeys.deadkey_cedilla;
            }
        }
        if let Some(diaeresis_bits) = layout.deadkeys.diaeresis_bits {
            if keycode == diaeresis_bits {
                return layout.deadkeys.deadkey_diaeresis;
            }
        }
        if let Some(grave_accent_bits) = layout.deadkeys.grave_accent_bits {
            if keycode == grave_accent_bits {
                return layout.deadkeys.deadkey_grave_accent;
            }
        }
        if let Some(circumflex_bits) = layout.deadkeys.circumflex_bits {
            if keycode == circumflex_bits {
                return layout.deadkeys.deadkey_circumflex;
            }
        }
        if let Some(tilde_bits) = layout.deadkeys.tilde_bits {
            if keycode == tilde_bits {
                return layout.deadkeys.deadkey_tilde;
            }
        }
        None
    })
}

// https://github.com/PaulStoffregen/cores/blob/master/usb_hid/usb_api.cpp#L196
/// Get required modifier key to type keycode
pub fn modifier_for_keycode(layout: &Layout, keycode: u16) -> u8 {
    let mut modifier = 0u16;

    if keycode & layout.shift_mask > 0 {
        modifier |= SHIFT_MODIFIER;
    }

    if let Some(alt_mask) = layout.alt_mask {
        if keycode & alt_mask > 0 {
            modifier |= RIGHT_ALT_MODIFIER;
        }
    }

    if let Some(ctrl_mask) = layout.ctrl_mask {
        if keycode & ctrl_mask > 0 {
            modifier |= RIGHT_CTRL_MODIFIER;
        }
    }

    modifier as u8
}

// https://github.com/PaulStoffregen/cores/blob/master/usb_hid/usb_api.cpp#L212
/// Get key for keycode
pub fn key_for_keycode(layout: &Layout, keycode: u16) -> u8 {
    let key = keycode & KEY_MASK;
    match layout.non_us {
        Some(non_us) => {
            if key == non_us {
                100u8
            } else {
                key as u8
            }
        }
        None => key as u8,
    }
}

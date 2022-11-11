#![doc = include_str!("../README.md")]


/// Keyboard module
pub mod key;

/// Key Translation Module
mod translate;

/// Mouse Module
pub mod mouse;


mod hid;
/// HID file module
pub use hid::HID;

//^.+?num:(\d+?), byte:(0x..), ktype:KeyOrigin::(.+?),.+?Char\(vec!\[(.+?)\]\)\}, | $4 => $2, // $1, $2, $3, $4
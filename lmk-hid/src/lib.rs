pub mod key;
pub mod mouse;
mod hid;
pub use hid::HID;

//^.+?num:(\d+?), byte:(0x..), ktype:KeyOrigin::(.+?),.+?Char\(vec!\[(.+?)\]\)\}, | $4 => $2, // $1, $2, $3, $4


#[cfg(test)]
mod tests {
    use crate::key::Modifier;

    #[test]
    fn test_modifiers() {
        println!("{:02x}", Modifier::LeftMeta.to_mkbyte());
    }
}


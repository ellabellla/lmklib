use gen_layouts_sys::LAYOUT_MAP;
use keyboard_layouts::{keycode_for_unicode, Keycode, deadkey_for_keycode, key_for_keycode, modifier_for_keycode};
use lazy_static::lazy_static;
use maplit::hashmap;
use pretty_assertions::assert_eq;
use tokio_linux_uhid::{Bus, CreateParams, UHIDDevice};

use std::collections::HashMap;
use std::panic;
use std::process::Command;
use std::thread;
use std::time::Duration;

use bytes::{BufMut, Bytes, BytesMut};
use log::debug;


// Keyboard Report Descriptor
const RDESC: [u8; 63] = [
    0x05, 0x01, 0x09, 0x06, 0xa1, 0x01, 0x05, 0x07, 0x19, 0xe0, 0x29, 0xe7, 0x15, 0x00, 0x25, 0x01,
    0x75, 0x01, 0x95, 0x08, 0x81, 0x02, 0x95, 0x01, 0x75, 0x08, 0x81, 0x03, 0x95, 0x05, 0x75, 0x01,
    0x05, 0x08, 0x19, 0x01, 0x29, 0x05, 0x91, 0x02, 0x95, 0x01, 0x75, 0x03, 0x91, 0x03, 0x95, 0x06,
    0x75, 0x08, 0x15, 0x00, 0x25, 0x65, 0x05, 0x07, 0x19, 0x00, 0x29, 0x65, 0x81, 0x00, 0xc0,
];

const ALPHA_NUMERIC: &'static str =
    "1234567890ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";
const SYMBOLS: &'static str = "\"#!$%&'()*+,-.\\/:;<=>?@[]^_`{|}~\"";

const HID_PACKET_SUFFIX: [u8; 5] = [0u8; 5];
const RELEASE_KEYS_HID_PACKET: [u8; 8] = [0u8; 8];
/// The number of bytes in a keyboard HID packet
pub const HID_PACKET_LEN: usize = 8;

#[derive(Debug)]
#[repr(u8)]
pub enum Release {
    All = 0,
    Keys = 1,
    None = 2,
}

#[derive(Debug)]
pub struct KeyMod {
    pub key: u8,
    pub modifier: u8,
    pub release: Release,
}


lazy_static! {
    static ref X_LAYOUT_MAP: HashMap<&'static str, (&'static str, Option<&'static str>)> = hashmap! {
        "LAYOUT_GERMAN" => ("de", None),
        "LAYOUT_PORTUGUESE_BRAZILIAN" => ("br", None),
        // latin9 enables dead grave accent
        "LAYOUT_FRENCH" => ("fr", Some("latin9")),
        "LAYOUT_US_ENGLISH" => ("us", None),
        "LAYOUT_FINNISH" => ("fi", None),
        // Fails because Linux is different from Windows for '#' so not changing
        "LAYOUT_SPANISH_LATIN_AMERICA" => ("latam",  None),
        "LAYOUT_FRENCH_BELGIAN" => ("be", None),
        // Fails because Linux is different from Windows for '`' so not changing
        "LAYOUT_IRISH" => ("ie", None),
        "LAYOUT_SWEDISH" => ("se", None),
        "LAYOUT_GERMAN_SWISS" => ("ch", None),
        "LAYOUT_CANADIAN_FRENCH" => ("ca", Some("fr")),
        "LAYOUT_SPANISH" => ("es", None),
        "LAYOUT_PORTUGUESE" => ("pt", None),
        "LAYOUT_ICELANDIC" => ("is", None),
        "LAYOUT_TURKISH" => ("tr", None),
        // For some reason the ' deadkey is used but not printed when testing
        "LAYOUT_US_INTERNATIONAL" => ("us", Some("intl")),
        // use canadian multix to be inline with Windows Canadian Multilingual standard
        "LAYOUT_CANADIAN_MULTILINGUAL" => ("ca", Some("multix")),
        "LAYOUT_FRENCH_SWISS" => ("ch", Some("fr")),
        "LAYOUT_DANISH" => ("dk", None),
        // Fails on keyboard layout not containing '`' and '~'
        "LAYOUT_ITALIAN" => ("it", None),
        // Fails miserably
        "LAYOUT_GERMAN_MAC" => ("de", Some("mac")),
        "LAYOUT_NORWEGIAN" => ("no", None),
        "LAYOUT_UNITED_KINGDOM" => ("gb", None),
    };
}

fn set_x_keyboard_layout(layout: &str, variant: Option<&str>) {
    let mut builder = Command::new("sudo");

    builder.args(&["localectl", "set-x11-keymap", layout]);
    eprintln!("Setting layout: {}", layout);

    if let Some(variant) = variant {
        builder.args(&["", variant]);
        eprintln!("Setting variant: {}", variant);
    }

    builder
        .output()
        .expect(&format!("Failed to set x keyboard layout for {}", layout));

    Command::new("sudo")
        .arg("setupcon")
        .output()
        .expect("Failed to setup console");
}


/// Get a list of the key and modifier pairs required to type the given string on a keyboard with
/// the specified layout.
pub fn string_to_keys_and_modifiers(layout_key: &str, string: &str) -> Option<Vec<KeyMod>> {
    let layout = LAYOUT_MAP
        .get(layout_key)?;

    let mut keys_and_modifiers: Vec<KeyMod> = Vec::with_capacity(string.len());

    for c in string.chars() {
        match keycode_for_unicode(layout, c as u16) {
            Keycode::ModifierKeySequence(modifier, sequence) => {
                for keycode in sequence {
                    keys_and_modifiers.push(KeyMod {
                        key: keycode as u8,
                        modifier: modifier as u8,
                        release: Release::Keys,
                    });
                }
                // Manually add release after sequence is finished
                keys_and_modifiers.push(KeyMod {
                    key: 0,
                    modifier: 0,
                    release: Release::None,
                });
            }
            Keycode::RegularKey(keycode) => {
                if let Some(dead_keycode) = deadkey_for_keycode(layout, keycode) {
                    let key = key_for_keycode(layout, dead_keycode);
                    let modifier = modifier_for_keycode(layout, dead_keycode);
                    keys_and_modifiers.push(KeyMod {
                        key,
                        modifier,
                        release: Release::All,
                    });
                }
                let key = key_for_keycode(layout, keycode);
                let modifier = modifier_for_keycode(layout, keycode);
                keys_and_modifiers.push(KeyMod {
                    key,
                    modifier,
                    release: Release::All,
                });
            }
            _ => return None,
        }
    }

    Some(keys_and_modifiers)
}

/// Create the sequence of HID packets required to type the given string. Impersonating a keyboard
/// with the specified layout. These packets can be written directly to a HID device file.
pub fn string_to_hid_packets(layout_key: &str, string: &str) -> Option<Bytes> {
    let keys_and_modifiers = string_to_keys_and_modifiers(layout_key, string)?;

    debug!("Keys and Modifiers for {}:{:?}", string, keys_and_modifiers);
    let mut packet_bytes = BytesMut::with_capacity(HID_PACKET_LEN * keys_and_modifiers.len() * 2);

    for KeyMod {
        key,
        modifier,
        release,
    } in keys_and_modifiers.iter()
    {
        packet_bytes.put_u8(*modifier);
        packet_bytes.put_u8(0u8);
        packet_bytes.put_u8(*key);
        packet_bytes.put_slice(&HID_PACKET_SUFFIX);
        match *release {
            Release::All => packet_bytes.put_slice(&RELEASE_KEYS_HID_PACKET),
            Release::Keys => {
                packet_bytes.put_u8(*modifier);
                packet_bytes.put_u8(0u8);
                packet_bytes.put_u8(0u8);
                packet_bytes.put_slice(&HID_PACKET_SUFFIX);
            }
            Release::None => {}
        }
    }

    Some(packet_bytes.freeze())
}

fn write_string_for_layout(string: &str, layout: &str) {
    let create_params = CreateParams {
        name: String::from("all_layouts_uhid"),
        phys: String::from(""),
        uniq: String::from(""),
        bus: Bus::USB,
        vendor: 0x15d9,
        product: 0x0a37,
        version: 0,
        country: 0,
        data: RDESC.to_vec(),
    };

    let core = tokio_core::reactor::Core::new().unwrap();
    let handle = core.handle();
    let mut uhid_device = UHIDDevice::create(&handle, create_params, None).unwrap();
    let mut input = String::new();

    let packets =
        string_to_hid_packets(layout, &format!("{}\n", string)).unwrap();

    uhid_device.send_input(&[0u8; 8]).unwrap();

    thread::sleep(Duration::from_millis(500));
    // helps when debugging testing to wait on enter being pressed in console
    //std::io::stdin().read_line(&mut input).unwrap();

    for packet in packets.chunks(8) {
        uhid_device.send_input(&packet).unwrap();
        thread::sleep(Duration::from_millis(50));
    }

    uhid_device.destroy().unwrap();

    std::io::stdin().read_line(&mut input).unwrap();

    assert_eq!(
        // removes internal spaces as linux does not honour the same deadkeys as mac/windows
        input.trim().replace(" ", ""),
        string,
        "Unexpected output for layout: {}",
        layout
    );
}

fn run_layout_test<T>(layout: &str, test: T) -> ()
where
    T: FnOnce() -> () + panic::UnwindSafe,
{
    let (x_layout, x_variant) = X_LAYOUT_MAP.get(layout).unwrap();
    set_x_keyboard_layout(x_layout, *x_variant);

    let result = panic::catch_unwind(|| test());

    set_x_keyboard_layout("gb", None);

    assert!(result.is_ok())
}

#[test]
#[ignore]
fn create_uhid_device() {
    let create_params = CreateParams {
        name: String::from("all_layouts_uhid"),
        phys: String::from(""),
        uniq: String::from(""),
        bus: Bus::USB,
        vendor: 0x15d9,
        product: 0x0a37,
        version: 0,
        country: 0,
        data: RDESC.to_vec(),
    };

    let core = tokio_core::reactor::Core::new().unwrap();
    let handle = core.handle();
    let mut _uhid_device = UHIDDevice::create(&handle, create_params, None).unwrap();
    loop {}
}

macro_rules! test_layout {
    ($f:ident, $l:ident, $s:ident) => {
        #[test]
        fn $f() {
            run_layout_test(stringify!($l), || {
                write_string_for_layout($s, stringify!($l));
            });
        }
    };
}

test_layout!(
    test_alphanumeric_layout_german,
    LAYOUT_GERMAN,
    ALPHA_NUMERIC
);
test_layout!(
    test_alphanumeric_layout_portuguese_brazilian,
    LAYOUT_PORTUGUESE_BRAZILIAN,
    ALPHA_NUMERIC
);
test_layout!(
    test_alphanumeric_layout_french,
    LAYOUT_FRENCH,
    ALPHA_NUMERIC
);
test_layout!(
    test_alphanumeric_layout_us_english,
    LAYOUT_US_ENGLISH,
    ALPHA_NUMERIC
);
test_layout!(
    test_alphanumeric_layout_finnish,
    LAYOUT_FINNISH,
    ALPHA_NUMERIC
);
test_layout!(
    test_alphanumeric_layout_spanish_latin_america,
    LAYOUT_SPANISH_LATIN_AMERICA,
    ALPHA_NUMERIC
);
test_layout!(
    test_alphanumeric_layout_french_belgian,
    LAYOUT_FRENCH_BELGIAN,
    ALPHA_NUMERIC
);
test_layout!(test_alphanumeric_layout_irish, LAYOUT_IRISH, ALPHA_NUMERIC);
test_layout!(
    test_alphanumeric_layout_swedish,
    LAYOUT_SWEDISH,
    ALPHA_NUMERIC
);
test_layout!(
    test_alphanumeric_layout_german_swiss,
    LAYOUT_GERMAN_SWISS,
    ALPHA_NUMERIC
);
test_layout!(
    test_alphanumeric_layout_canadian_french,
    LAYOUT_CANADIAN_FRENCH,
    ALPHA_NUMERIC
);
test_layout!(
    test_alphanumeric_layout_spanish,
    LAYOUT_SPANISH,
    ALPHA_NUMERIC
);
test_layout!(
    test_alphanumeric_layout_portuguese,
    LAYOUT_PORTUGUESE,
    ALPHA_NUMERIC
);
test_layout!(
    test_alphanumeric_layout_icelandic,
    LAYOUT_ICELANDIC,
    ALPHA_NUMERIC
);
test_layout!(
    test_alphanumeric_layout_turkish,
    LAYOUT_TURKISH,
    ALPHA_NUMERIC
);
test_layout!(
    test_alphanumeric_layout_us_international,
    LAYOUT_US_INTERNATIONAL,
    ALPHA_NUMERIC
);
test_layout!(
    test_alphanumeric_layout_canadian_multilingual,
    LAYOUT_CANADIAN_MULTILINGUAL,
    ALPHA_NUMERIC
);
test_layout!(
    test_alphanumeric_layout_french_swiss,
    LAYOUT_FRENCH_SWISS,
    ALPHA_NUMERIC
);
test_layout!(
    test_alphanumeric_layout_danish,
    LAYOUT_DANISH,
    ALPHA_NUMERIC
);
test_layout!(
    test_alphanumeric_layout_italian,
    LAYOUT_ITALIAN,
    ALPHA_NUMERIC
);
test_layout!(
    test_alphanumeric_layout_german_mac,
    LAYOUT_GERMAN_MAC,
    ALPHA_NUMERIC
);
test_layout!(
    test_alphanumeric_layout_norwegian,
    LAYOUT_NORWEGIAN,
    ALPHA_NUMERIC
);
test_layout!(
    test_alphanumeric_layout_united_kingdom,
    LAYOUT_UNITED_KINGDOM,
    ALPHA_NUMERIC
);
test_layout!(test_symbols_layout_german, LAYOUT_GERMAN, SYMBOLS);
test_layout!(
    test_symbols_layout_portuguese_brazilian,
    LAYOUT_PORTUGUESE_BRAZILIAN,
    SYMBOLS
);
test_layout!(test_symbols_layout_french, LAYOUT_FRENCH, SYMBOLS);
test_layout!(test_symbols_layout_us_english, LAYOUT_US_ENGLISH, SYMBOLS);
test_layout!(test_symbols_layout_finnish, LAYOUT_FINNISH, SYMBOLS);
test_layout!(
    test_symbols_layout_spanish_latin_america,
    LAYOUT_SPANISH_LATIN_AMERICA,
    SYMBOLS
);
test_layout!(
    test_symbols_layout_french_belgian,
    LAYOUT_FRENCH_BELGIAN,
    SYMBOLS
);
test_layout!(test_symbols_layout_irish, LAYOUT_IRISH, SYMBOLS);
test_layout!(test_symbols_layout_swedish, LAYOUT_SWEDISH, SYMBOLS);
test_layout!(
    test_symbols_layout_german_swiss,
    LAYOUT_GERMAN_SWISS,
    SYMBOLS
);
test_layout!(
    test_symbols_layout_canadian_french,
    LAYOUT_CANADIAN_FRENCH,
    SYMBOLS
);
test_layout!(test_symbols_layout_spanish, LAYOUT_SPANISH, SYMBOLS);
test_layout!(test_symbols_layout_portuguese, LAYOUT_PORTUGUESE, SYMBOLS);
test_layout!(test_symbols_layout_icelandic, LAYOUT_ICELANDIC, SYMBOLS);
test_layout!(test_symbols_layout_turkish, LAYOUT_TURKISH, SYMBOLS);
test_layout!(
    test_symbols_layout_us_international,
    LAYOUT_US_INTERNATIONAL,
    SYMBOLS
);
test_layout!(
    test_symbols_layout_canadian_multilingual,
    LAYOUT_CANADIAN_MULTILINGUAL,
    SYMBOLS
);
test_layout!(
    test_symbols_layout_french_swiss,
    LAYOUT_FRENCH_SWISS,
    SYMBOLS
);
test_layout!(test_symbols_layout_danish, LAYOUT_DANISH, SYMBOLS);
test_layout!(test_symbols_layout_italian, LAYOUT_ITALIAN, SYMBOLS);
test_layout!(test_symbols_layout_german_mac, LAYOUT_GERMAN_MAC, SYMBOLS);
test_layout!(test_symbols_layout_norwegian, LAYOUT_NORWEGIAN, SYMBOLS);
test_layout!(
    test_symbols_layout_united_kingdom,
    LAYOUT_UNITED_KINGDOM,
    SYMBOLS
);

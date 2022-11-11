#![warn(missing_docs)]

use std::{
    io::{self},
    str::FromStr,
    time::Duration,
};

use gen_layouts_sys::*;
use keyboard_layouts::{keycode_for_unicode, Keycode, deadkey_for_keycode, key_for_keycode, modifier_for_keycode};

pub use crate::translate::*;
use crate::HID;

const KEY_PACKET_KEY_LEN: usize = 32;
const KEY_PACKET_LEN: usize = KEY_PACKET_KEY_IDX + KEY_PACKET_KEY_LEN;
const KEY_PACKET_MOD_IDX: usize = 0;
const KEY_PACKET_KEY_IDX: usize = 1;

#[derive(Debug, Clone)]
/// LED State Types
pub enum LEDState {
    /// Kana
    Kana,
    /// Compose
    Compose,
    /// ScrollLock
    ScrollLock,
    /// CapsLock
    CapsLock,
    /// NumLock
    NumLock,
}

/// Abstraction for LED State Packets
pub struct LEDStatePacket {
    data: u8,
}

impl LEDStatePacket {
    /// New blank LED state packet
    pub fn new() -> LEDStatePacket {
        LEDStatePacket { data: 0x00 }
    }

    /// Create a new LED State Packet from an incoming raw packet.
    pub fn new_from_packet(hid: &mut HID, timeout: Duration) -> io::Result<LEDStatePacket> {
        Ok(LEDStatePacket {
            data: hid.receive_states_packet(timeout)?.unwrap_or(0),
        })
    }

    /// Get the state of a LED State Type.
    /// True means on
    /// False means off
    pub fn get_state(&self, state: &LEDState) -> bool {
        match state {
            LEDState::Kana => self.data & self.data & (0x01 << 4) != 0,
            LEDState::Compose => self.data & (0x01 << 3) != 0,
            LEDState::ScrollLock => self.data & (0x01 << 2) != 0,
            LEDState::CapsLock => self.data & (0x01 << 1) != 0,
            LEDState::NumLock => self.data & (0x01) != 0,
        }
    }

    /// Update LED States with an incoming raw packet with a timeout.
    pub fn update(&mut self, hid: &mut HID, timeout: Duration) -> io::Result<()> {
        match hid.receive_states_packet(timeout)? {
            Some(data) => self.data = data,
            None => (),
        }
        Ok(())
    }
}

#[derive(Debug, Eq, Hash, PartialEq, Clone, Copy)]
/// Basic Key Press
pub enum BasicKey {
    /// Key from Char
    Char(char, KeyOrigin),
    /// Special Key
    Special(SpecialKey),
}

/// Virtual Keyboard
pub struct Keyboard {
    packets: Vec<KeyPacket>,
    holding: KeyPacket,
    led_states: LEDStatePacket,
}

impl FromStr for Keyboard {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut keyboard = Keyboard::new();
        keyboard.press_basic_string(s);
        Ok(keyboard)
    }
}

impl Keyboard {
   /// New
   pub fn new() -> Keyboard {
      Keyboard {
         packets: Vec::new(),
         holding: KeyPacket::new(),
         led_states: LEDStatePacket::new(),
      }
   }

   /// Get a list of the supported keyboard layouts
   pub fn available_layouts() -> Vec<&'static str> {
      LAYOUT_MAP.keys().map(|k| *k).collect()
   }

   /// Get layout by key
   fn get_layout(layout_key: &str) -> Option<&'static Layout> {
      LAYOUT_MAP
         .get(layout_key)
   }

   /// Get the current LED state
   pub fn led_state(&self, state: &LEDState) -> bool {
      self.led_states.get_state(state)
   }

   /// update LED states from incoming led state packets
   pub fn update_led_state(&mut self, hid: &mut HID, timeout: Duration) -> io::Result<()> {
      self.led_states.update(hid, timeout)
   }

   fn add_buffer(&mut self, packet: &KeyPacket) {
      if let Some(last) = self.packets.last() {
         if last.contains_any(packet) {
               self.packets.push(self.create_release_packet())
         }
      }
   }

   /// Hold key down
   pub fn hold_key(&mut self, key: &BasicKey) -> Option<u8> {
      #[cfg(feature = "debug")]
      {
         println!("hold {:?}", key);
      }
      let kbytes = match key {
         BasicKey::Char(c, key_origin) => c.to_kbytes(key_origin)?,
         BasicKey::Special(special) => [0, special.to_kbyte()],
      };
      self.holding.add_key(&kbytes);
      self.packets.push(self.create_release_packet());
      Some(kbytes[1])
   }

   /// Release Key
   pub fn release_key(&mut self, key: &BasicKey) {
      #[cfg(feature = "debug")]
      {
         println!("release {:?}", key);
      }
      let kbytes = match key {
         BasicKey::Char(c, key_origin) => match c.to_kbytes(key_origin) {
               Some(kbytes) => kbytes,
               None => return,
         },
         BasicKey::Special(special) => [0, special.to_kbyte()],
      };
      self.holding.remove_key(&kbytes);
      self.packets.push(self.create_release_packet());
   }

   /// Hold all keys in string
   pub fn hold_string(&mut self, str: &str) {
      #[cfg(feature = "debug")]
      {
         println!("hold {:?}", str);
      }
      for c in str.chars() {
         let kbytes = match c.to_kbytes(&KeyOrigin::Keyboard) {
               Some(packet) => packet,
               None => continue,
         };
         self.holding.add_key(&kbytes);
      }
      self.packets.push(self.create_release_packet());
   }

   /// Release all keys in string
   pub fn release_string(&mut self, str: &str) {
      #[cfg(feature = "debug")]
      {
         println!("release {:?}", str);
      }
      for c in str.chars() {
         let kbytes = match c.to_kbytes(&KeyOrigin::Keyboard) {
               Some(packet) => packet,
               None => continue,
         };
         self.holding.remove_key(&kbytes);
      }
      self.packets.push(self.create_release_packet());
   }

   /// Hold key with keycode
   pub fn hold_keycode(&mut self, key: u8) {
      #[cfg(feature = "debug")]
      {
         println!("hold {:08b}", key);
      }
      self.holding.add_key(&[0, key]);
      self.packets.push(self.create_release_packet());
   }

   /// Release key with keycode
   pub fn release_keycode(&mut self, key: u8) {
      #[cfg(feature = "debug")]
      {
         println!("release {:08b}", key);
      }
      self.holding.remove_key(&[0, key]);
      self.packets.push(self.create_release_packet());
   }

   /// Hold modifier key
   pub fn hold_mod(&mut self, modifier: &Modifier) {
      #[cfg(feature = "debug")]
      {
         println!("hold {:?}", modifier);
      }
      self.holding.push_modifier(modifier);
      self.packets.push(self.create_release_packet());
   }

   /// Release modifier key
   pub fn release_mod(&mut self, modifier: &Modifier) {
      #[cfg(feature = "debug")]
      {
         println!("release {:?}", modifier);
      }
      self.holding.remove_mod(modifier);
      self.packets.push(self.create_release_packet());
   }

   fn add_held_keys(&mut self, packet: &mut KeyPacket) {
      let mut i = 0;
      for byte in &mut self.holding.data {
         *byte |= packet.data[i];
         i += 1;
      }
   }

   fn create_release_packet(&self) -> KeyPacket {
      self.holding.clone()
   }

   /// Press
   pub fn press(&mut self, layout_key: &str, c: char) -> Option<()> {
      let layout = Keyboard::get_layout(layout_key)?;
      match keycode_for_unicode(layout, c as u16) {
            Keycode::ModifierKeySequence(modifier, sequence) => {
               let mut packet = KeyPacket::from_mod_keycode(modifier as  u8);
               for keycode in sequence {
                  packet.push_key_keycode(keycode as u8);
               }
               self.add_buffer(&packet);
               self.add_held_keys(&mut packet);
               self.packets.push(packet);
               self.packets.push(self.create_release_packet());
            },
            Keycode::RegularKey(keycode) => {
               if let Some(dead_keycode) = deadkey_for_keycode(layout, keycode) {
                  let key = key_for_keycode(layout, dead_keycode);
                  let modifier = modifier_for_keycode(layout, dead_keycode);

                  let mut packet = KeyPacket::from_keycodes(modifier, key);
                  self.add_buffer(&packet);
                  self.add_held_keys(&mut packet);
                  self.packets.push(packet);

                  self.packets.push(self.create_release_packet());
               }
               let key = key_for_keycode(layout, keycode);
               let modifier = modifier_for_keycode(layout, keycode);

               let mut packet = KeyPacket::from_keycodes(modifier, key);
               self.add_held_keys(&mut packet);
               self.packets.push(packet);

               self.packets.push(self.create_release_packet());
            }
            _ => return None,
      }
      #[cfg(feature = "debug")]
      {
         println!("press {:?}", c);
      }
      Some(())
   }

   /// Send keystroke in packet
   pub fn press_packet(&mut self, mut packet: KeyPacket) {
      self.add_held_keys(&mut packet);
      self.packets.push(packet)
   }

   /// Send modifier keystroke
   pub fn press_modifier(&mut self, modifier: &Modifier) {
      #[cfg(feature = "debug")]
      {
         println!("press {:?}", modifier);
      }
      let mut packet = self.create_release_packet();
      packet.push_modifier(modifier);
      self.packets.push(packet);
      self.packets.push(self.create_release_packet());
   }

   /// Send shortcut keystroke
   pub fn press_shortcut(&mut self, modifiers: &[Modifier], key: &BasicKey) -> Option<()> {
      #[cfg(feature = "debug")]
      {
         println!("press {:?} {:?}", modifiers, key);
      }
      let mut packet = self.create_release_packet();
      for modifier in modifiers {
         packet.push_modifier(modifier);
      }
      packet.push_key(key);
      self.packets.push(self.create_release_packet());
      self.packets.push(packet);
      self.packets.push(self.create_release_packet());

      Some(())
   }

   fn press_special(&mut self, special: &SpecialKey) {
      #[cfg(feature = "debug")]
      {
         println!("press {:?}", special);
      }
      let mut packet = self.create_release_packet();
      packet.push_special(special);
      self.add_buffer(&packet);
      self.packets.push(packet);
   }

   fn press_char(&mut self, c: &char, key_origin: &KeyOrigin) -> Option<()> {
      #[cfg(feature = "debug")]
      {
         println!("press {:?} {:?}", c, key_origin);
      }
      let mut packet = KeyPacket::from_char(&c, key_origin)?;
      packet.push_char(c, key_origin);
      self.add_buffer(&packet);
      self.packets.push(packet);
      Some(())
   }

   /// Send keystroke
   pub fn press_key(&mut self, key: &BasicKey) -> Option<()> {
      match key {
         BasicKey::Char(c, key_origin) => self.press_char(c, key_origin)?,
         BasicKey::Special(special) => self.press_special(special),
      }
      Some(())
   }

   /// Send keystroke of keycode
   pub fn press_keycode(&mut self, key: u8) {
      #[cfg(feature = "debug")]
      {
         println!("press {:08b}", key);
      }
      let mut packet = KeyPacket::new();
      packet.add_key(&[0, key]);
      self.add_buffer(&packet);
      self.packets.push(packet);
   }

   /// Send keystrokes of keys in string
   pub fn press_basic_string(&mut self, str: &str) {
      #[cfg(feature = "debug")]
      {
         println!("press {:?}", str);
      }
      for c in str.chars() {
         let mut packet = self.create_release_packet();
         let kbytes = match c.to_kbytes(&KeyOrigin::Keyboard) {
               Some(packet) => packet,
               None => continue,
         };
         packet.add_key(&kbytes);
         let needs_space = packet.get_key(&kbytes);
         self.packets.push(packet);

         if needs_space {
               self.packets.push(self.create_release_packet())
         }
      }
   }

   /// Send keystrokes of keys in string
   pub fn press_string(&mut self, layout_key: &str, str: &str) {
      #[cfg(feature = "debug")]
      {
         println!("press {:?}", str);
      }
      for c in str.chars() {
         self.press(layout_key, c);
      }
   }

   /// Flush Buffered keystrokes to HID interface
   pub fn send(&mut self, hid: &mut HID) -> io::Result<usize> {
      if self.packets.len() == 0 {
         return Ok(0);
      }

      self.packets.push(self.create_release_packet());
      let res = KeyPacket::send_all(&self.packets, hid);
      self.packets.clear();
      res
   }

   /// Send Buffered keystrokes to HID interface and keep buffered keystrokes
   pub fn send_keep(&self, hid: &mut HID) -> io::Result<usize> {
      if self.packets.len() == 0 {
         return Ok(0);
      }

      let res = KeyPacket::send_all(&self.packets, hid)?;
      let res2 = hid.send_key_packet(&self.create_release_packet().data)?;
      Ok(res + res2)
   }
}

/// Key Packet abstraction
pub struct KeyPacket {
    data: [u8; KEY_PACKET_LEN],
}

impl KeyPacket {
   /// New
   pub fn new() -> KeyPacket {
      KeyPacket {
         data: [0x00; KEY_PACKET_LEN],
      }
   }

   fn add_key(&mut self, kbytes: &[u8; 2]) {
      self.data[KEY_PACKET_MOD_IDX] |= kbytes[0];
      self.data[KEY_PACKET_KEY_IDX + usize::try_from(kbytes[1] >> 3).unwrap_or(0)] |=
         1 << (kbytes[1] & 0x7);
   }

   fn remove_key(&mut self, kbytes: &[u8; 2]) {
      self.data[KEY_PACKET_MOD_IDX] &= !kbytes[0];
      self.data[KEY_PACKET_KEY_IDX + usize::try_from(kbytes[1] >> 3).unwrap_or(0)] &=
         !(1 << (kbytes[1] & 0x7));
   }

   fn get_key(&self, kbytes: &[u8; 2]) -> bool {
      self.data[KEY_PACKET_KEY_IDX + usize::try_from(kbytes[1] >> 3).unwrap_or(0)]
         & (1 << (kbytes[1] & 0x7))
         != 0
   }

   fn add_mod(&mut self, modifier: &Modifier) {
      self.data[KEY_PACKET_MOD_IDX] |= modifier.to_mkbyte();
   }

   fn remove_mod(&mut self, modifier: &Modifier) {
      self.data[KEY_PACKET_MOD_IDX] &= !modifier.to_mkbyte();
   }

   /// Create from keycodes
   pub fn from_keycodes(modifier: u8, key: u8) -> KeyPacket {
      let mut packet = KeyPacket::new();
      packet.push_modifier_key_keycode(modifier, key);
      packet
   }

   /// Create from modifier keycode
   pub fn from_mod_keycode(modifier: u8) -> KeyPacket {
      let mut packet = KeyPacket::new();
      packet.push_modifier_keycode(modifier);
      packet
   }

   /// Create from key lists
   pub fn from_list(modifiers: &[Modifier], keys: &[(char, KeyOrigin); 6]) -> KeyPacket {
      let mut packet = KeyPacket::new();
      packet.data[KEY_PACKET_MOD_IDX] = Modifier::all_to_byte(modifiers);
      for (c, key_origin) in keys.iter() {
         if let Some(kbytes) = c.to_kbytes(key_origin) {
               packet.add_key(&kbytes)
         }
      }
      packet
   }

   /// Create from char
   pub fn from_char(c: &char, key_origin: &KeyOrigin) -> Option<KeyPacket> {
      let mut packet = KeyPacket::new();
      let kbytes = c.to_kbytes(key_origin)?;
      packet.add_key(&kbytes);
      Some(packet)
   }

   /// Create from special key
   pub fn from_special(special: &SpecialKey) -> KeyPacket {
      let mut packet = KeyPacket::new();
      let kbytes = special.to_kbyte();
      packet.add_key(&[0x0, kbytes]);
      packet
   }

   /// Check if packet contains the keystroke for a char
   pub fn contains_char(&self, key: char, key_origin: &KeyOrigin) -> bool {
      let kbyte = match key.to_kbytes(key_origin) {
         Some(kbytes) => kbytes[1],
         None => return false,
      };
      self.contains_kbyte(&kbyte)
   }

   /// Check if packet contains the keystroke in a given packet
   pub fn contains_any(&self, packet: &KeyPacket) -> bool {
      for i in KEY_PACKET_KEY_IDX..KEY_PACKET_LEN {
         if packet.data[i] & self.data[i] != 0{
               return true;
         }
      }

      return false;
   }

   /// Check if packet contains special key
   pub fn contains_special(&self, special: &SpecialKey) -> bool {
      self.contains_kbyte(&special.to_kbyte())
   }

   fn contains_kbyte(&self, kbyte: &u8) -> bool {
      for i in KEY_PACKET_KEY_IDX..(KEY_PACKET_KEY_LEN + KEY_PACKET_KEY_IDX) {
         if self.data[i] == *kbyte {
               return true;
         }
      }

      return false;
   }

   /// Add modifier to packet
   pub fn push_modifier(&mut self, modifier: &Modifier) {
      self.add_mod(modifier)
   }

   /// Add key from keycode to packet
   pub fn push_key_keycode(&mut self, key: u8) {
      self.add_key(&[0x00, key]);
   }

   /// Add modifier from keycode to packet
   pub fn push_modifier_keycode(&mut self, modifier: u8) {
      self.add_key(&[modifier, 0x00]);
   }

   /// Add modifier & key from keycodes to packet
   pub fn push_modifier_key_keycode(&mut self, modifier: u8, key: u8) {
      self.add_key(&[modifier, key]);
   }

   /// Add key to packet
   pub fn push_key(&mut self, key: &BasicKey) -> Option<u8> {
      match key {
         BasicKey::Char(c, key_origin) => self.push_char(c, key_origin),
         BasicKey::Special(special) => self.push_special(special),
      }
   }

   /// Add char to packet
   pub fn push_char(&mut self, key: &char, key_origin: &KeyOrigin) -> Option<u8> {
      let kbytes = key.to_kbytes(key_origin)?;
      self.add_key(&kbytes);
      Some(kbytes[1])
   }

   /// Add special key to packet
   pub fn push_special(&mut self, special: &SpecialKey) -> Option<u8> {
      let kbytes = special.to_kbyte();
      self.add_key(&[0x0, kbytes]);
      Some(kbytes)
   }

   /// Send packet to hid interface
   pub fn send(&self, hid: &mut HID) -> io::Result<usize> {
      hid.send_key_packet(&self.data)
   }

   /// Send a list of packets to hid interface
   pub fn send_all(packets: &Vec<KeyPacket>, hid: &mut HID) -> io::Result<usize> {
      let mut size = 0;
      for packet in packets {
         size += packet.send(hid)?;
      }

      Ok(size)
   }

   /// Print packet data
   pub fn print_data(data: &[u8]) {
      for data in data {
         print!("{:02x}", data);
      }
      println!();
   }

   /// Print packet
   pub fn print_packet(packet: &KeyPacket) {
      for data in packet.data {
         print!("{:02x}", data);
      }
      println!();
   }

   /// Print packets
   pub fn print_packets(packets: &Vec<KeyPacket>) {
      for packet in packets {
         for data in packet.data {
               print!("{:02x}", data);
         }
         println!();
      }
   }

   fn clone(&self) -> KeyPacket {
      KeyPacket {
         data: self.data.clone(),
      }
   }
}

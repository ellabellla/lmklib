use std::{sync::Arc, io, fmt::Display};

use async_trait::async_trait;
use log::error;
use tokio::{sync::{RwLock, mpsc::{UnboundedSender, self}, oneshot}, runtime::Handle};
use uinput::{event::{self, controller::Mouse, relative::{Position, Wheel}, keyboard::{Key, Misc, KeyPad, InputAssist}}, Device};
use virt_hid::{key::{self, BasicKey, KeyOrigin, SpecialKey, Modifier}, mouse::{self, MouseDir, MouseButton}};

use crate::{OrLogIgnore, OrLog, modules::ModuleManager};

use super::{Function, FunctionInterface, ReturnCommand, FunctionType, FunctionConfig, FunctionConfigData, State, StateHelpers};

#[derive(Debug)]
/// HID Error
pub enum HIDError {
    /// UInput error
    UInput(uinput::Error),
    /// IO error
    IO(io::Error),
    /// No configuration for HID
    NoConfig,
    /// Message passing error
    ChannelError,
}

impl Display for HIDError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HIDError::UInput(e) => f.write_fmt(format_args!("UInput error, {}", e)),
            HIDError::IO(e) => f.write_fmt(format_args!("IO error, {}", e)),
            HIDError::NoConfig => f.write_str("No configuration was supplied"),
            HIDError::ChannelError => f.write_str("Channel Error"),
        }
    }
}

/// Switch hid mode (uinput, usb hid)
pub struct SwitchHid {
    name: String,
    prev_state: u16,
    hid: Arc<RwLock<HID>>,
}

impl SwitchHid {
    /// new
    pub fn new(name: String, hid: Arc<RwLock<HID>>) -> Function {
        Some(Box::new(SwitchHid{name, prev_state: 0, hid}))
    }
}

#[async_trait]
impl FunctionInterface for SwitchHid {
    async fn event(&mut self, state: State) -> ReturnCommand {
        if state.rising(self.prev_state) {
            self.hid.read().await.switch(self.name.clone());
        }

        self.prev_state = state;
        ReturnCommand::None
    }

    fn ftype(&self) -> FunctionType {
        FunctionType::SwitchHid{name:self.name.clone()}
    }
}

#[derive(Debug)]
/// Hid command for wrapped usb hid and uinput controller
enum Command {
    HoldKey(char),
    HoldSpecial(SpecialKey),
    HoldModifier(Modifier),
    ReleaseKey(char),
    ReleaseSpecial(SpecialKey),
    ReleaseModifier(Modifier),
    PressBasicStr(String),
    PressStr(String, String),
    ScrollWheel(i8),
    MoveMouse(i8, MouseDir),
    HoldButton(MouseButton),
    ReleaseButton(MouseButton),
    SendKeyboard,
    SendMouse,
    Switch(String),
}

/// HID controller
pub struct HID {
    tx: UnboundedSender<Command>,
    led: String, 
    mouse: String, 
    keyboard: String,
}

#[async_trait]
impl FunctionConfig for HID {
    type Output = Arc<RwLock<HID>>;

    type Error = HIDError;

    fn to_config_data(&self) -> super::FunctionConfigData {
        FunctionConfigData::HID{mouse: self.mouse.clone(), keyboard: self.keyboard.clone(), led: self.led.clone()}
    }

    async fn from_config(function_config: &super::FunctionConfiguration) -> Result<Self::Output, Self::Error> {
        let Some(FunctionConfigData::HID { mouse, keyboard, led }) = function_config
            .get(|config| matches!(config, FunctionConfigData::HID { mouse: _, keyboard: _, led: _})) else {
                return Err(HIDError::NoConfig)
        };
        HID::new(mouse.clone(), keyboard.clone(), led.clone(), function_config.module_manager.clone()).await
    }
}

impl HID {
    /// New, requires path to usb hid interfaces
    pub async fn new(mouse: String, keyboard: String, led: String, module_manager: Arc<ModuleManager>) -> Result<Arc<RwLock<HID>>, HIDError> {
        let (tx, mut rx) = mpsc::unbounded_channel();        
        let (new_tx, new_rx) = oneshot::channel();    

        let mse = mouse.clone();
        let kbd = keyboard.clone();
        let ld = led.clone();
        tokio::task::spawn_blocking(move || {
            let mut hid = match virt_hid::HID::new(&mse, &kbd, &ld){
                Ok(hid) => hid,
                Err(e) => {new_tx.send(Err(HIDError::IO(e))).or_log_ignore("Broken Channel (HID Driver)"); return;}
            };

            let mut uinput = match (|| -> Result<Device, HIDError>{
                uinput::default().map_err(|e| HIDError::UInput(e))?
                    .name("lmk").map_err(|e| HIDError::UInput(e))?
                    .event(event::Keyboard::All).map_err(|e| HIDError::UInput(e))?
                    .event(event::Controller::Mouse(Mouse::Left)).map_err(|e| HIDError::UInput(e))?
                    .event(event::Controller::Mouse(Mouse::Right)).map_err(|e| HIDError::UInput(e))?
                    .event(event::Controller::Mouse(Mouse::Middle)).map_err(|e| HIDError::UInput(e))?
                    .event(event::Relative::Position(Position::X)).map_err(|e| HIDError::UInput(e))?
                    .event(event::Relative::Position(Position::Y)).map_err(|e| HIDError::UInput(e))?
                    .event(event::Relative::Wheel(Wheel::Vertical)).map_err(|e| HIDError::UInput(e))?
                    .create()
                    .map_err(|e| HIDError::UInput(e))
            })() {
                Ok(uinput) => uinput,
                Err(e) => {new_tx.send(Err(e)).or_log_ignore("Broken Channel (HID Driver)"); return;}
            };
            new_tx.send(Ok(())).or_log_ignore("Broken Channel (HID Driver)");

            let mut keyboard = key::Keyboard::new(); 
            let mut mouse = mouse::Mouse::new();
            let mut cur_hid = "usb".to_owned();

            while let Some(command) = rx.blocking_recv() {
                match command {
                    Command::HoldKey(key) => match cur_hid.as_ref() { 
                        "usb" => {
                            keyboard.hold_key(&BasicKey::Char(key, KeyOrigin::Keyboard));
                        } 
                        "uinput" => {
                            let Some(key) = char_to_uinput(key) else {
                                continue;
                            };
                            uinput.press(&key).or_log("Uinput error (HID Driver)");
                        }
                        _ => {Handle::current().block_on(module_manager.hold_key(&cur_hid, key)).or_log("Unable to process hid input (HID Driver)");}
                    },
                    Command::HoldSpecial(special) => match cur_hid.as_ref() { 
                        "usb" => {
                            keyboard.hold_key(&BasicKey::Special(special));
                        } 
                        "uinput" => {
                            let Some(key) = special_to_uinput(special) else {
                                continue;
                            };
                            uinput.press(&key).or_log("Uinput error (HID Driver)");
                        }
                        _ => {Handle::current().block_on(module_manager.hold_special(&cur_hid, special)).or_log("Unable to process hid input (HID Driver)");}
                    },
                    Command::HoldModifier(modifier) => match cur_hid.as_ref() { 
                        "usb" => {
                            keyboard.hold_mod(&modifier);
                        } 
                        "uinput" => {
                            let Some(key) = mod_to_uinput(modifier) else {
                                continue;
                            };
                            uinput.press(&key).or_log("Uinput error (HID Driver)");
                        }
                        _ => {Handle::current().block_on(module_manager.hold_modifier(&cur_hid, modifier)).or_log("Unable to process hid input (HID Driver)");}
                    },
                    Command::ReleaseKey(key) => match cur_hid.as_ref() { 
                        "usb" => {
                            keyboard.release_key(&BasicKey::Char(key, KeyOrigin::Keyboard));
                        } 
                        "uinput" => {
                            let Some(key) = char_to_uinput(key) else {
                                continue;
                            };
                            uinput.release(&key).or_log("Uinput error (HID Driver)");
                        }
                        _ => {Handle::current().block_on(module_manager.release_key(&cur_hid, key)).or_log("Unable to process hid input (HID Driver)");}
                    },
                    Command::ReleaseSpecial(special) => match cur_hid.as_ref() { 
                        "usb" => {
                            keyboard.release_key(&BasicKey::Special(special));
                        } 
                        "uinput" => {
                            let Some(key) = special_to_uinput(special) else {
                                continue;
                            };
                            uinput.release(&key).or_log("Uinput error (HID Driver)");
                        }
                        _ => {Handle::current().block_on(module_manager.release_special(&cur_hid, special)).or_log("Unable to process hid input (HID Driver)");}
                    },
                    Command::ReleaseModifier(modifier) => match cur_hid.as_ref() { 
                        "usb" => {
                            keyboard.release_mod(&modifier);
                        } 
                        "uinput" => {
                            let Some(key) = mod_to_uinput(modifier) else {
                                continue;
                            };
                            uinput.release(&key).or_log("Uinput error (HID Driver)");
                        }
                        _ => {Handle::current().block_on(module_manager.release_modifier(&cur_hid, modifier)).or_log("Unable to process hid input (HID Driver)");}
                    },
                    Command::PressBasicStr(str) => match cur_hid.as_ref() { 
                        "usb" => {
                            keyboard.press_basic_string(&str);
                        } 
                        "uinput" => {
                            for key in str.chars() {
                                if requires_shift(key) {
                                    uinput.press(&event::Keyboard::Key(Key::LeftShift)).or_log("Uinput error (HID Driver)");
                                }
                
                                let Some(ukey) = char_to_uinput(key) else {
                                    continue;
                                };
                                uinput.click(&ukey).or_log("Uinput error (HID Driver)");
                
                                if requires_shift(key) {
                                    uinput.release(&event::Keyboard::Key(Key::LeftShift)).or_log("Uinput error (HID Driver)");
                                }
                            }
                        }
                        _ => {Handle::current().block_on(module_manager.press_basic_str(&cur_hid, str)).or_log("Unable to process hid input (HID Driver)");}
                    },
                    Command::PressStr(layout, str) => match cur_hid.as_ref() { 
                        "usb" => {
                            keyboard.press_string(&layout, &str);
                        } 
                        "uinput" => {
                            for key in str.chars() {
                                if requires_shift(key) {
                                    uinput.press(&event::Keyboard::Key(Key::LeftShift)).or_log("Uinput error (HID Driver)");
                                }
                
                                let Some(ukey) = char_to_uinput(key) else {
                                    continue;
                                };
                                uinput.click(&ukey).or_log("Uinput error (HID Driver)");
                
                                if requires_shift(key) {
                                    uinput.release(&event::Keyboard::Key(Key::LeftShift)).or_log("Uinput error (HID Driver)");
                                }
                            }
                        }
                        _ => {Handle::current().block_on(module_manager.press_str(&cur_hid, layout, str)).or_log("Unable to process hid input (HID Driver)");}
                    }
                    Command::ScrollWheel(amount) => match cur_hid.as_ref() { 
                        "usb" => {
                            mouse.scroll_wheel(&amount);
                        } 
                        "uinput" => {
                            uinput.position(&event::Relative::Wheel(Wheel::Vertical), amount as i32).or_log("Uinput error (HID Driver)");
                        }
                        _ => {Handle::current().block_on(module_manager.scroll_wheel(&cur_hid, amount)).or_log("Unable to process hid input (HID Driver)");}
                    },
                    Command::MoveMouse(amount, dir) => match cur_hid.as_ref() { 
                        "usb" => {
                            mouse.move_mouse(&amount, &dir);
                        } 
                        "uinput" => {
                            let dir = mouse_dir_to_position(dir);
                            uinput.position(&event::Relative::Position(dir), amount as i32).or_log("Uinput error (HID Driver)");
                        }
                        _ => {Handle::current().block_on(module_manager.move_mouse(&cur_hid, amount, dir)).or_log("Unable to process hid input (HID Driver)");}
                    },
                    Command::HoldButton(button) => match cur_hid.as_ref() { 
                        "usb" => {
                            mouse.hold_button(&button);
                        } 
                        "uinput" => {
                            let button = mouse_button_to_mouse(button);
                            uinput.press(&event::Controller::Mouse(button)).or_log("Uinput error (HID Driver)");
                        }
                        _ => {Handle::current().block_on(module_manager.hold_button(&cur_hid, button)).or_log("Unable to process hid input (HID Driver)");}
                    },
                    Command::ReleaseButton(button) => match cur_hid.as_ref() { 
                        "usb" => {
                            mouse.release_button(&button);
                        } 
                        "uinput" => {
                            let button = mouse_button_to_mouse(button);
                            uinput.release(&event::Controller::Mouse(button)).or_log("Uinput error (HID Driver)");
                        }
                        _ => {Handle::current().block_on(module_manager.release_button(&cur_hid, button)).or_log("Unable to process hid input (HID Driver)");}
                    },
                    Command::SendKeyboard => match cur_hid.as_ref() { 
                        "usb" => {
                            keyboard.send(&mut hid).or_log("USB HID error (HID Driver)");
                        }
                        "uinput" => (),
                        _ => {Handle::current().block_on(module_manager.send_keyboard(&cur_hid)).or_log("Unable to process hid input (HID Driver)");}
                    },
                    Command::SendMouse => match cur_hid.as_ref() { 
                        "usb" => {
                            mouse.send(&mut hid).or_log("USB HID error (HID Driver)");
                        }
                        "uinput" => (),
                        _ => {Handle::current().block_on(module_manager.send_mouse(&cur_hid)).or_log("Unable to process hid input (HID Driver)");}
                    },
                    Command::Switch(name) => match name.as_ref() {
                        "usb" => cur_hid = name,
                        "uinput" => cur_hid = name,
                        _ => if module_manager.is_hid(&name) {
                            cur_hid = name;
                        } else {
                            error!("Could not switch to hid (HID Driver), Unable to find hid module, {}", name)
                        },
                    },
                }
            }
        });

        
        match new_rx.await {
            Ok(res) => res.map(|_| Arc::new(RwLock::new(HID { tx, mouse, keyboard, led }))),
            Err(_) => Err(HIDError::ChannelError)
        }
    }

    /// Hold key
    pub async fn hold_key(&self, key: char) {
        self.tx.send(Command::HoldKey(key)).or_log_ignore("Broken Channel (HID Driver)");
    }

    /// Release key
    pub async fn release_key(&self, key: char) {
        self.tx.send(Command::ReleaseKey(key)).or_log_ignore("Broken Channel (HID Driver)");
    }

    /// Hold special
    pub async fn hold_special(&self, special: SpecialKey) {
        self.tx.send(Command::HoldSpecial(special)).or_log_ignore("Broken Channel (HID Driver)");
    }

    /// Release special
    pub async fn release_special(&self, special: SpecialKey) {
        self.tx.send(Command::ReleaseSpecial(special)).or_log_ignore("Broken Channel (HID Driver)");
    }

    /// Hold mod
    pub async fn hold_mod(&self, modifier: Modifier) {
        self.tx.send(Command::HoldModifier(modifier)).or_log_ignore("Broken Channel (HID Driver)");
    }

    /// Release mod
    pub async fn release_mod(&self, modifier: Modifier) {
        self.tx.send(Command::ReleaseModifier(modifier)).or_log_ignore("Broken Channel (HID Driver)");
    }

    /// Type string (with default keyboard layout)
    pub async fn press_basic_string(&self, str: &str)  {
        self.tx.send(Command::PressBasicStr(str.to_string())).or_log_ignore("Broken Channel (HID Driver)");
    }

    /// Press string resolving keys with a keyboard layout (uinput ignores the layout)
    pub async fn press_string(&self, layout: &str, str: &str)  {
        self.tx.send(Command::PressStr(layout.to_string(), str.to_string())).or_log_ignore("Broken Channel (HID Driver)");
    }

    /// Scroll wheel
    pub async fn scroll_wheel(&self, amount: i8) {
        self.tx.send(Command::ScrollWheel(amount)).or_log_ignore("Broken Channel (HID Driver)");
    }

    /// Move mouse
    pub async fn move_mouse(&self, amount: i8, dir: MouseDir) {
        self.tx.send(Command::MoveMouse(amount, dir)).or_log_ignore("Broken Channel (HID Driver)");
    }

    /// Hold button
    pub async fn hold_button(&self, button: MouseButton) {
        self.tx.send(Command::HoldButton(button)).or_log_ignore("Broken Channel (HID Driver)");
    }
    
    /// Release button
    pub async fn release_button(&self, button: MouseButton) {
        self.tx.send(Command::ReleaseButton(button)).or_log_ignore("Broken Channel (HID Driver)");
    }
    
    /// Send key strokes to interface (does nothing for uinput)
    pub fn send_keyboard(&self) {
        self.tx.send(Command::SendKeyboard).or_log_ignore("Broken Channel (HID Driver)");
    }
    
    /// Send mouse packets to interface (does nothing for uinput)
    pub fn send_mouse(&self) {
        self.tx.send(Command::SendMouse).or_log_ignore("Broken Channel (HID Driver)");
    }

    /// Switch hid controllers (uinput or usb hid)
    pub fn switch(&self, name: String) {
        self.tx.send(Command::Switch(name)).or_log_ignore("Broken Channel (HID Driver)");
    }
}

/// Usb mouse to uinput
fn mouse_button_to_mouse(button: MouseButton) -> Mouse {
    match button {
        MouseButton::Left => Mouse::Left,
        MouseButton::Right => Mouse::Right,
        MouseButton::Middle => Mouse::Middle,
    }
}

/// Usb mouse to uinput
fn mouse_dir_to_position(dir: MouseDir) -> Position {
    match dir {
        MouseDir::X => Position::X,
        MouseDir::Y => Position::Y,
    }
}

/// True if character requires shift to press
fn requires_shift(key: char) -> bool {
    match key {
        '!' | '@' | '#' | '$' | '%' | '^' | '&' | '*' | '(' | ')' | '_' | '+' | '{' | '}' | '|' | ':' | '"' | '<' | '>' | '?' | '~' => true, 
        _ => key.is_ascii_alphabetic() && key.is_ascii_uppercase(),
    }
}

/// Usb keyboard to uinput
fn mod_to_uinput(modifier: Modifier) -> Option<event::Keyboard> {
    Some(match modifier {
        Modifier::LeftControl => event::Keyboard::Key(Key::LeftControl),
        Modifier::LeftShift => event::Keyboard::Key(Key::LeftShift),
        Modifier::LeftAlt => event::Keyboard::Key(Key::LeftAlt),
        Modifier::LeftMeta => event::Keyboard::Key(Key::LeftMeta),
        Modifier::RightControl => event::Keyboard::Key(Key::RightControl),
        Modifier::RightShift => event::Keyboard::Key(Key::RightShift),
        Modifier::RightAlt => event::Keyboard::Key(Key::RightAlt),
        Modifier::RightMeta => event::Keyboard::Key(Key::RightMeta),
    })
}

/// Usb keyboard to uinput
fn special_to_uinput(special: SpecialKey) -> Option<event::Keyboard> {
    Some(match special {
        SpecialKey::ReturnEnter => event::Keyboard::Key(Key::Enter),
        SpecialKey::Return => event::Keyboard::Key(Key::LineFeed),
        SpecialKey::Escape => event::Keyboard::Key(Key::Esc),
        SpecialKey::Backspace => event::Keyboard::Key(Key::BackSpace),
        SpecialKey::Tab => event::Keyboard::Key(Key::Tab),
        SpecialKey::Spacebar => event::Keyboard::Key(Key::Space),
        SpecialKey::NONUSHashAndTilda => return None,
        SpecialKey::CapsLock => event::Keyboard::Key(Key::CapsLock),
        SpecialKey::F1 => event::Keyboard::Key(Key::F1),
        SpecialKey::F2 => event::Keyboard::Key(Key::F2),
        SpecialKey::F3 => event::Keyboard::Key(Key::F3),
        SpecialKey::UpArrow => event::Keyboard::Key(Key::Up),
        SpecialKey::DownArrow => event::Keyboard::Key(Key::Down),
        SpecialKey::LeftArrow => event::Keyboard::Key(Key::Left),
        SpecialKey::RightArrow => event::Keyboard::Key(Key::Right),
        SpecialKey::PageDown => event::Keyboard::Key(Key::PageDown),
        SpecialKey::End => event::Keyboard::Key(Key::End),
        SpecialKey::DeleteForward => event::Keyboard::Key(Key::Delete),
        SpecialKey::PageUp => event::Keyboard::Key(Key::PageUp),
        SpecialKey::Home => event::Keyboard::Key(Key::Home),
        SpecialKey::Insert => event::Keyboard::Key(Key::Insert),
        SpecialKey::Pause => event::Keyboard::Misc(Misc::Pause),
        SpecialKey::ScrollLock => event::Keyboard::Key(Key::ScrollLock),
        SpecialKey::PrintScreen => return None,
        SpecialKey::F12 => event::Keyboard::Key(Key::F12),
        SpecialKey::F11 => event::Keyboard::Key(Key::F11),
        SpecialKey::F10 => event::Keyboard::Key(Key::F10),
        SpecialKey::F9 => event::Keyboard::Key(Key::F9),
        SpecialKey::F8 => event::Keyboard::Key(Key::F8),
        SpecialKey::F7 => event::Keyboard::Key(Key::F7),
        SpecialKey::F6 => event::Keyboard::Key(Key::F6),
        SpecialKey::F5 => event::Keyboard::Key(Key::F5),
        SpecialKey::F4 => event::Keyboard::Key(Key::F4),
        SpecialKey::NumLockAndClear => event::Keyboard::Key(Key::NumLock),
        SpecialKey::Enter => event::Keyboard::Key(Key::Enter),
        SpecialKey::Application => event::Keyboard::Misc(Misc::AppSelect),
        SpecialKey::Power => event::Keyboard::Misc(Misc::Power),
        SpecialKey::RightGUI => event::Keyboard::Key(Key::RightMeta),
        SpecialKey::RightAlt => event::Keyboard::Key(Key::RightAlt),
        SpecialKey::RightShift => event::Keyboard::Key(Key::RightShift),
        SpecialKey::RightControl => event::Keyboard::Key(Key::RightControl),
        SpecialKey::LeftGUI => event::Keyboard::Key(Key::LeftMeta),
        SpecialKey::LeftAlt => event::Keyboard::Key(Key::LeftAlt),
        SpecialKey::LeftShift => event::Keyboard::Key(Key::LeftShift),
        SpecialKey::LeftControl => event::Keyboard::Key(Key::LeftControl),
        SpecialKey::Hexadecimal => return None,
        SpecialKey::Decimal => return None,
        SpecialKey::Octal => return None,
        SpecialKey::Binary => return None,
        SpecialKey::ClearEntry => return None,
        SpecialKey::Clear => event::Keyboard::Misc(Misc::Clear),
        SpecialKey::PlusMinux => event::Keyboard::KeyPad(KeyPad::PlusMinus),
        SpecialKey::MemoryDivide => return None,
        SpecialKey::MemoryMultiply => return None,
        SpecialKey::MemorySubtract => return None,
        SpecialKey::MemoryAdd => return None,
        SpecialKey::MemoryClear => return None,
        SpecialKey::MemoryRecall => return None,
        SpecialKey::MemoryStore => return None,
        SpecialKey::Space => event::Keyboard::Key(Key::Space),
        SpecialKey::Or => return None,
        SpecialKey::And => return None,
        SpecialKey::XOR => return None,
        SpecialKey::CurrencySubunit => return None,
        SpecialKey::CurrencyUnit => return None,
        SpecialKey::DecimalSeparator => return None,
        SpecialKey::ThousandsSeparator => return None,
        SpecialKey::_000 => return None,
        SpecialKey::_00 => return None,
        SpecialKey::ExSel => return None,
        SpecialKey::CrSelProps => return None,
        SpecialKey::ClearAgain => return None,
        SpecialKey::Oper => return None,
        SpecialKey::Out => return None,
        SpecialKey::Separator => return None,
        SpecialKey::Prior => return None,
        SpecialKey::Cancel => event::Keyboard::InputAssist(InputAssist::Cancel),
        SpecialKey::SysReqAttention1 => return None,
        SpecialKey::AlternateErase => return None,
        SpecialKey::LANG9 => return None,
        SpecialKey::LANG8 => return None,
        SpecialKey::LANG7 => return None,
        SpecialKey::LANG6 => return None,
        SpecialKey::LANG5 => return None,
        SpecialKey::LANG4 => return None,
        SpecialKey::LANG3 => return None,
        SpecialKey::LANG2 => return None,
        SpecialKey::LANG1 => return None,
        SpecialKey::International9 => return None,
        SpecialKey::International8 => return None,
        SpecialKey::International7 => return None,
        SpecialKey::International6 => return None,
        SpecialKey::International5 => return None,
        SpecialKey::International4 => return None,
        SpecialKey::International3 => return None,
        SpecialKey::International2 => return None,
        SpecialKey::International1 => return None,
        SpecialKey::LockingScrollLock => return None,
        SpecialKey::LockingNumLock => return None,
        SpecialKey::LockingCapsLock => return None,
        SpecialKey::VolumeDown => event::Keyboard::Misc(Misc::VolumeDown),
        SpecialKey::VolumeUp => event::Keyboard::Misc(Misc::VolumeUp),
        SpecialKey::Mute => event::Keyboard::Misc(Misc::Mute),
        SpecialKey::Find => event::Keyboard::Misc(Misc::Find),
        SpecialKey::Paste => event::Keyboard::Misc(Misc::Paste),
        SpecialKey::Copy => event::Keyboard::Misc(Misc::Copy),
        SpecialKey::Cut => event::Keyboard::Misc(Misc::Cut),
        SpecialKey::Undo => event::Keyboard::Misc(Misc::Undo),
        SpecialKey::Again => event::Keyboard::Misc(Misc::Again),
        SpecialKey::Stop => event::Keyboard::Misc(Misc::Stop),
        SpecialKey::Select => event::Keyboard::Misc(Misc::Select),
        SpecialKey::Menu => event::Keyboard::Misc(Misc::Menu),
        SpecialKey::Help => event::Keyboard::Misc(Misc::Help),
        SpecialKey::Execute => return None,
        SpecialKey::F24 => event::Keyboard::Key(Key::F24),
        SpecialKey::F23 => event::Keyboard::Key(Key::F23),
        SpecialKey::F22 => event::Keyboard::Key(Key::F22),
        SpecialKey::F21 => event::Keyboard::Key(Key::F21),
        SpecialKey::F20 => event::Keyboard::Key(Key::F20),
        SpecialKey::F19 => event::Keyboard::Key(Key::F19),
        SpecialKey::F18 => event::Keyboard::Key(Key::F18),
        SpecialKey::F17 => event::Keyboard::Key(Key::F17),
        SpecialKey::F16 => event::Keyboard::Key(Key::F16),
        SpecialKey::F15 => event::Keyboard::Key(Key::F15),
        SpecialKey::F14 => event::Keyboard::Key(Key::F14),
        SpecialKey::F13 => event::Keyboard::Key(Key::F13),
        SpecialKey::NonUSSlashAndPipe => return  None,
        SpecialKey::_DotAndDelete => event::Keyboard::KeyPad(KeyPad::Dot),
        SpecialKey::_0AndInsert => event::Keyboard::KeyPad(KeyPad::_0),
        SpecialKey::_9AndPageUp => event::Keyboard::KeyPad(KeyPad::_9),
        SpecialKey::_8AndUpArrow => event::Keyboard::KeyPad(KeyPad::_8),
        SpecialKey::_7AndHome => event::Keyboard::KeyPad(KeyPad::_7),
        SpecialKey::_6AndRightArrow => event::Keyboard::KeyPad(KeyPad::_6),
        SpecialKey::_5 => event::Keyboard::KeyPad(KeyPad::_5),
        SpecialKey::_4AndLeftArrow => event::Keyboard::KeyPad(KeyPad::_4),
        SpecialKey::_3AndPageDn => event::Keyboard::KeyPad(KeyPad::_3),
        SpecialKey::_2AndDownArrow => event::Keyboard::KeyPad(KeyPad::_2),
        SpecialKey::_1AndEnd => event::Keyboard::KeyPad(KeyPad::_1),
        SpecialKey::PadClear => return None,
        SpecialKey::PadBackspace => return None,
        SpecialKey::PadTab => return None,
        SpecialKey::EqualsSign => return None,
        SpecialKey::Comma => event::Keyboard::KeyPad(KeyPad::Comma),
    })
} 

/// Usb keyboard to uinput
fn char_to_uinput(key: char) -> Option<event::Keyboard> {
    Some(match key.to_ascii_lowercase() {
		'1' | '!' => event::Keyboard::Key(Key::_1),
		'2' | '@' => event::Keyboard::Key(Key::_2),
		'3' | '#' => event::Keyboard::Key(Key::_3),
		'4' | '$' => event::Keyboard::Key(Key::_4),
		'5' | '%' => event::Keyboard::Key(Key::_5),
		'6' | '^' => event::Keyboard::Key(Key::_6),
		'7' | '&' => event::Keyboard::Key(Key::_7),
		'8' | '*' => event::Keyboard::Key(Key::_8),
		'9' | '(' => event::Keyboard::Key(Key::_9),
		'0' | ')' => event::Keyboard::Key(Key::_0),
		'-' | '_' => event::Keyboard::Key(Key::Minus),
		'=' | '+' => event::Keyboard::Key(Key::Equal),
		'q' => event::Keyboard::Key(Key::Q),
		'w' => event::Keyboard::Key(Key::W),
		'e' => event::Keyboard::Key(Key::E),
		'r' => event::Keyboard::Key(Key::R),
		't' => event::Keyboard::Key(Key::T),
		'y' => event::Keyboard::Key(Key::Y),
		'u' => event::Keyboard::Key(Key::U),
		'i' => event::Keyboard::Key(Key::I),
		'o' => event::Keyboard::Key(Key::O),
		'p' => event::Keyboard::Key(Key::P),
		'{' | '[' => event::Keyboard::Key(Key::LeftBrace),
		'}' | ']' => event::Keyboard::Key(Key::RightBrace),
		'a' => event::Keyboard::Key(Key::A),
		's' => event::Keyboard::Key(Key::S),
		'd' => event::Keyboard::Key(Key::D),
		'f' => event::Keyboard::Key(Key::F),
		'g' => event::Keyboard::Key(Key::G),
		'h' => event::Keyboard::Key(Key::H),
		'j' => event::Keyboard::Key(Key::J),
		'k' => event::Keyboard::Key(Key::K),
		'l' => event::Keyboard::Key(Key::L),
		';' | ':' => event::Keyboard::Key(Key::SemiColon),
		'\'' | '"' => event::Keyboard::Key(Key::Apostrophe),
		'~' | '`'  => event::Keyboard::Key(Key::Grave),
		'\\' | '|' => event::Keyboard::Key(Key::BackSlash),
		'z' => event::Keyboard::Key(Key::Z),
		'x' => event::Keyboard::Key(Key::X),
		'c' => event::Keyboard::Key(Key::C),
		'v' => event::Keyboard::Key(Key::V),
		'b' => event::Keyboard::Key(Key::B),
		'n' => event::Keyboard::Key(Key::N),
		'm' => event::Keyboard::Key(Key::M),
		',' | '<' => event::Keyboard::Key(Key::Comma),
		'.' | '>' => event::Keyboard::Key(Key::Dot),
		'/' | '?' => event::Keyboard::Key(Key::Slash),
		' ' => event::Keyboard::Key(Key::Space),
        _ => return None,
    })
}
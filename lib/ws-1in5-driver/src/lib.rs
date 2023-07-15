#![doc = include_str!("../README.md")]

use std::{
    collections::HashMap,
    io::{BufReader, BufRead},
    process::{Child, Command, Stdio},
    sync::Arc,
    time::{Duration, Instant}, thread, fmt::Display, path::PathBuf, fs, env,
};

use abi_stable::{
    export_root_module, prefix_type::PrefixTypeTrait, sabi_extern_fn, sabi_trait::TD_Opaque,
    std_types::RString,
};
use chrono::Utc;
use image::DynamicImage;
use imageproc::drawing;
use itertools::Itertools;
use key_module::hid::{HIDBox, HidModule, HidModuleRef, HID};
use key_rpc::{Client, ClientError};
use rusttype::{Font, Scale};
use serde::{Serialize, Deserialize};
use serde_json::Value;
use tokio::{runtime::{Handle, Runtime}, sync::{Mutex, mpsc::{UnboundedReceiver, self, UnboundedSender}}, task::JoinHandle};
use virt_hid::key::{Modifier, SpecialKey};
use ws_1in5_i2c::{WS1in5, OLED_HEIGHT, OLED_WIDTH};

pub trait OrPrint<T> {
    fn or_log(self, msg: &str) -> Option<T>;
}

impl<T, E> OrPrint<T> for Result<T, E> 
where
    E: Display
{
    fn or_log(self, msg: &str) -> Option<T> {
        match self {
            Ok(res) => Some(res),
            Err(e) => {
                println!("{}, {}", msg, e);
                None
            },
        }
    }
}

#[export_root_module]
pub fn get_library() -> HidModuleRef {
    HidModule { new_hid }.leak_into_prefix()
}

#[sabi_extern_fn]
fn new_hid() -> HIDBox {
    HIDBox::from_value(ScreenHid::new(), TD_Opaque)
}

fn parse_index(chr: char) -> Option<usize> {
    match chr {
        '0' => Some(0),
        '1' => Some(1),
        '2' => Some(2),
        '3' => Some(3),
        '4' => Some(4),
        '5' => Some(5),
        '6' => Some(6),
        '7' => Some(7),
        '8' => Some(8),
        '9' => Some(9),
        _ => None,
    }
}

fn rpc<F, U>(f: F) -> Result<U, ClientError>
where
    F: FnOnce(Client) -> Result<U, ClientError>,
{
    Client::new("ipc:///lmk/ksf.ipc").and_then(f)
}

fn get_key(coord: (usize, usize), layer: String) -> Result<String, String> {
    let value: Value = serde_json::from_str(&layer).map_err(|e| e.to_string())?;
    let Value::Array(y_axis) = value else {
        return Err("Malformed layer rpc response".to_string())
    };

    let Some(row) = y_axis.get(coord.1) else {
        return Err("Malformed layer rpc response".to_string())
    };

    let Value::Array(row) = row else {
        return Err("Malformed layer rpc response".to_string())
    };

    let Some(func) = row.get(coord.0) else {
        return Err("Malformed layer rpc response".to_string())
    };

    serde_json::to_string(func).map_err(|e| e.to_string())
}

#[derive(Clone)]
pub struct Resources {
    pub font: Font<'static>,
    pub scale10: Scale,
    pub scale12: Scale,
    pub scale20: Scale,
    pub scale30: Scale,
}

#[derive(Debug, PartialEq, Eq, Hash, Clone, Copy)]
pub enum StateType {
    Home,
    Variables,
    Term,
    Empty,
}

#[derive(Serialize, Deserialize)]
pub struct SystemData {
    ducky_dir: Option<PathBuf>,
    bork_dir: Option<PathBuf>,
    working_dir: Option<PathBuf>,
}

impl SystemData {
    fn new() -> SystemData {
        SystemData { ducky_dir: None, bork_dir: None, working_dir: None }
    }

    fn ducky_scripts(&self) -> Vec<(String, String)> {
        fs::read_dir(self.ducky())
        .and_then(|dir| 
            Ok(
                dir.into_iter()
                .filter_map(|file| 
                    file.map(|file| (
                        file.file_name().to_string_lossy().to_string(), 
                        file.path().to_string_lossy().to_string()
                    )).ok()
                ).filter(|(name, _)| name.ends_with(".duck"))
                .collect::<Vec<(String, String)>>()
            )
        ).unwrap_or_else(|_| vec![])
    }

    fn ducky(&self) -> PathBuf {
        self.ducky_dir.clone().unwrap_or_else(|| self.working_path())
    }

    fn bork_scripts(&self) -> Vec<(String, String)> {
        fs::read_dir(self.bork())
        .and_then(|dir| 
            Ok(
                dir.into_iter()
                .filter_map(|file| 
                    file.map(|file| (
                        file.file_name().to_string_lossy().to_string(), 
                        file.path().to_string_lossy().to_string()
                    )).ok()
                ).filter(|(name, _)| name.ends_with(".bork"))
                .collect::<Vec<(String, String)>>()
            )
        ).unwrap_or_else(|_| vec![])
    }
    
    fn bork(&self) -> PathBuf {
        self.bork_dir.clone().unwrap_or_else(|| self.working_path())
    }

    fn working_path(&self) -> PathBuf {
        self.working_dir.clone().unwrap_or_else(|| {
            env::var("HOME")
            .and_then(|path| Ok(PathBuf::from(path)))
            .or_else(|_| env::current_dir()).unwrap_or_default()
        })
    }
}

pub struct ScreenHid {
    shift: bool,

    rt: Runtime,
    _handle: JoinHandle<()>,

    states: Arc<Mutex<HashMap<StateType, Box<dyn State>>>>,
    last_interact: Arc<Mutex<Instant>>,
    curr_state: Arc<Mutex<StateType>>,

    system_data: SystemData,
}

pub struct  CallbackCommand {
    state: StateType,
    msg: u32
}

async fn spawn(
    screen: Arc<Mutex<WS1in5>>, 
    last_interact: Arc<Mutex<Instant>>, 
    mut command_pipe: UnboundedReceiver<CallbackCommand>,
    states: Arc<Mutex<HashMap<StateType, Box<dyn State>>>>,
    curr_state: Arc<Mutex<StateType>>, 
) {
    let rt = tokio::runtime::Handle::current();

    let mut last_reset = *last_interact.lock().await;
    *last_interact.lock().await = Instant::now();
    let mut interval = tokio::time::interval(Duration::from_millis(250));

    loop {
        interval.tick().await;

        let lock_last_interact = last_interact.lock().await;
        if last_reset != *lock_last_interact
            && Instant::now() - *lock_last_interact >= Duration::from_secs(30)
        {
            let screen = screen.clone();
            tokio::task::spawn_blocking(move || {
                screen.blocking_lock().clear_all().ok();
            })
            .await
            .ok();
            last_reset = *lock_last_interact;
        }
        drop(lock_last_interact);

        while let Ok(CallbackCommand{state, msg}) = command_pipe.try_recv()  {
            if state == *curr_state.lock().await {
                states.lock().await.get_mut(&state).and_then(|state| {
                    Some(state.callback(msg, &rt))
                });
            }
        }
    }
}

impl ScreenHid {
    pub fn new() -> ScreenHid {
        let screen = Arc::new(Mutex::new(
            WS1in5::new(0x3c, 3, 22).expect("Screen initialization"),
        ));

        let font_data: &[u8] = include_bytes!("../font/NotoSansMono-VariableFont_wdth,wght.ttf");
        let font: Font<'static> = Font::try_from_bytes(font_data).expect("Valid font");

        let scale10 = Scale::uniform(10.0);

        let scale12 = Scale::uniform(12.0);

        let scale20 = Scale::uniform(20.0);

        let scale30 = Scale::uniform(30.0);

        let resources = Arc::new(Resources {
            font,
            scale10,
            scale12,
            scale20,
            scale30,
        });

        let (command_pipe_send, command_pipe_rev) = mpsc::unbounded_channel();
        
        let mut states: HashMap<StateType, Box<dyn State>> = HashMap::new();
        states.insert(StateType::Home, Box::new(HomeState::new(resources.clone(), screen.clone())));
        states.insert(
            StateType::Variables,
            Box::new(VariablesState::new(screen.clone(), resources.clone())),
        );
        states.insert(StateType::Term, Box::new(TermState::new(screen.clone(), resources.clone(), command_pipe_send)));
        states.insert(StateType::Empty, Box::new(EmptyState::new(screen.clone())));
        let states = Arc::new(Mutex::new(states));

        let last_interact = Arc::new(Mutex::new(Instant::now()));
        let curr_state = Arc::new(Mutex::new(StateType::Home));
        
        let rt = tokio::runtime::Runtime::new().unwrap();
        let handle = rt.spawn(spawn(
            screen.clone(), 
            last_interact.clone(), 
            command_pipe_rev, 
            states.clone(), 
            curr_state.clone(), 
        ));
        
        let shid = ScreenHid {
            shift: false,
            states,
            last_interact,
            curr_state,
            _handle: handle,
            rt: rt,
            system_data: SystemData::new(),
        };

        let curr_state = shid.curr_state.blocking_lock();
        shid.states.blocking_lock().get_mut(&curr_state).and_then(|state| {
            state.enter(
                *curr_state,
                &shid.rt.handle(),
            );
            state.draw(&shid.rt.handle());
            Some(())
        });
        drop(curr_state);
        shid
    }

    fn change_state(&mut self, next_state: StateType) {
        let mut curr_state = self.curr_state.blocking_lock();

        let mut states = self.states.blocking_lock();
        if states.contains_key(&next_state) {
            states.get_mut(&curr_state).and_then(|state| {
                Some(state.exit(next_state, &self.rt.handle()))
            });
            states.get_mut(&next_state).and_then(|state| {
                state.enter(
                    *curr_state,
                    &self.rt.handle(),
                );
                state.draw(&self.rt.handle());
                Some(())
            });
            *curr_state = next_state;
        }
    }
}

impl HID for ScreenHid {
    fn configure(&mut self, data: RString) {
        let Some(system_data): Option<SystemData> = serde_json::from_str(&data).or_log("Unable to parse configuration data") else {
            return;
        };

        self.system_data = system_data;
    }

    fn hold_key(&mut self, mut key: u32) {
        *self.last_interact.blocking_lock() = Instant::now();

        if self.shift {
            if let Some(mut char) = char::from_u32(key as u32) {
                if char.is_alphabetic() {
                    char = char.to_ascii_uppercase();
                } else {
                    char = match char {
                        '`' => '~',
                        '1' => '!',
                        '2' => '@',
                        '3' => '#',
                        '4' => '$',
                        '5' => '%',
                        '6' => '^',
                        '7' => '&',
                        '8' => '*',
                        '9' => '(',
                        '0' => ')',
                        '-' => '_',
                        '=' => '+',
                        '[' => '{',
                        ']' => '}',
                        '\\' => '|',
                        ';' => ':',
                        '\'' => '"',
                        ',' => '<',
                        '.' => '>',
                        '/' => '?',
                        _ => char,
                    };
                }
                key = char as u32;
            };
        }

        let curr_state = self.curr_state.blocking_lock();
        self.states.blocking_lock().get_mut(&curr_state).and_then(|state| {
            Some(state.hold_key(key, &self.rt.handle()))
        });
    }
    fn hold_special(&mut self, special: u32) {
        *self.last_interact.blocking_lock() = Instant::now();

        let curr_state = self.curr_state.blocking_lock();
        self.states.blocking_lock().get_mut(&curr_state).and_then(|state| {
            Some(state.hold_special(special, &self.rt.handle()))
        });
    }
    fn hold_modifier(&mut self, modifier: u32) {
        *self.last_interact.blocking_lock() = Instant::now();

        if modifier == Modifier::LeftShift as u32 || modifier == Modifier::RightShift as u32 {
            self.shift = true;
        }

        let curr_state = self.curr_state.blocking_lock();
        self.states.blocking_lock().get_mut(&curr_state).and_then(|state| {
            Some(state.hold_modifier(modifier, &self.rt.handle()))
        });
    }

    fn release_key(&mut self, key: u32) {
        *self.last_interact.blocking_lock() = Instant::now();

        let curr_state = self.curr_state.blocking_lock();
        self.states.blocking_lock().get_mut(&curr_state).and_then(|state| {
            Some(state.release_key(key, &self.rt.handle()))
        });
    }
    fn release_special(&mut self, special: u32) {
        let curr_state = self.curr_state.blocking_lock();
        self.states.blocking_lock().get_mut(&curr_state).and_then(|state| {
            Some(state.release_special(special, &self.rt.handle()))
        });
    }
    fn release_modifier(&mut self, modifier: u32) {
        *self.last_interact.blocking_lock() = Instant::now();

        if modifier == Modifier::LeftShift as u32 || modifier == Modifier::RightShift as u32 {
            self.shift = false;
        }

        let curr_state = self.curr_state.blocking_lock();
        self.states.blocking_lock().get_mut(&curr_state).and_then(|state| {
            Some(state.release_modifier(modifier, &self.rt.handle()))
        });
    }

    fn press_basic_str(&mut self, str: RString) {
        *self.last_interact.blocking_lock() = Instant::now();

        let curr_state = self.curr_state.blocking_lock();
        self.states.blocking_lock().get_mut(&curr_state).and_then(|state| {
            Some(state.press_basic_str(str, &self.rt.handle()))
        });
    }

    fn press_str(&mut self, _: RString, str: RString) {
        self.press_basic_str(str)
    }

    fn scroll_wheel(&mut self, amount: i8) {
        *self.last_interact.blocking_lock() = Instant::now();

        let curr_state = self.curr_state.blocking_lock();
        self.states.blocking_lock().get_mut(&curr_state).and_then(|state| {
            Some(state.scroll_wheel(amount, &self.rt.handle()))
        });
    }

    fn move_mouse_x(&mut self, amount: i8) {
        *self.last_interact.blocking_lock() = Instant::now();

        let curr_state = self.curr_state.blocking_lock();
        self.states.blocking_lock().get_mut(&curr_state).and_then(|state| {
            Some(state.move_mouse_x(amount, &self.rt.handle()))
        });
    }
    fn move_mouse_y(&mut self, amount: i8) {
        *self.last_interact.blocking_lock() = Instant::now();

        let curr_state = self.curr_state.blocking_lock();
        self.states.blocking_lock().get_mut(&curr_state).and_then(|state| {
            Some(state.move_mouse_y(amount, &self.rt.handle()))
        });
    }

    fn hold_button(&mut self, button: u32) {
        *self.last_interact.blocking_lock() = Instant::now();

        let curr_state = self.curr_state.blocking_lock();
        self.states.blocking_lock().get_mut(&curr_state).and_then(|state| {
            Some(state.hold_button(button, &self.rt.handle()))
        });
    }
    fn release_button(&mut self, button: u32) {
        *self.last_interact.blocking_lock() = Instant::now();

        let curr_state = self.curr_state.blocking_lock();
        self.states.blocking_lock().get_mut(&curr_state).and_then(|state| {
            Some(state.release_button(button, &self.rt.handle()))
        });
    }

    fn send_keyboard(&mut self) {}

    fn send_mouse(&mut self) {}

    fn send_command(&mut self, data: RString) {
        *self.last_interact.blocking_lock() = Instant::now();
        match data.as_str() {
            "wake" => {
            if *self.curr_state.blocking_lock() != StateType::Empty {
                    self.states.blocking_lock().get_mut(&self.curr_state.blocking_lock()).and_then(|state| {
                        Some(state.draw(&self.rt.handle()))
                    });
                } else {
                    self.change_state(StateType::Home);
                }
            }
            "home" => {
                self.change_state(StateType::Home);
            }
            "variables" => {
                self.change_state(StateType::Variables);
            }
            "term" => {
                self.change_state(StateType::Term);
            }
            "exit" => {
                self.change_state(StateType::Empty);
            }
            _ => {}
        }
    }
}


pub trait State: Send + Sync {
    fn draw(&mut self, _rt: &Handle) {
    }

    fn enter(
        &mut self,
        _prev: StateType,
        _rt: &Handle,
    ) {
    }
    fn exit(
        &mut self,
        _next: StateType,
        _rt: &Handle,
    ) {
    }

    fn hold_key(
        &mut self,
        _key: u32,
        _rt: &Handle,
    ) {
    }
    fn hold_special(
        &mut self,
        _special: u32,
        _rt: &Handle,
    ) {
    }
    fn hold_modifier(
        &mut self,
        _modifier: u32,
        _rt: &Handle,
    ) {
    }
    fn release_key(
        &mut self,
        _key: u32,
        _rt: &Handle,
    ) {
    }
    fn release_special(
        &mut self,
        _special: u32,
        _rt: &Handle,
    ) {
    }
    fn release_modifier(
        &mut self,
        _modifier: u32,
        _rt: &Handle,
    ) {
    }
    fn press_basic_str(
        &mut self,
        _str: RString,
        _rt: &Handle,
    ) {
    }
    fn scroll_wheel(
        &mut self,
        _amount: i8,
        _rt: &Handle,
    ) {
    }
    fn move_mouse_x(
        &mut self,
        _amount: i8,
        _rt: &Handle,
    ) {
    }
    fn move_mouse_y(
        &mut self,
        _amount: i8,
        _rt: &Handle,
    ) {
    }
    fn hold_button(
        &mut self,
        _button: u32,
        _rt: &Handle,
    ) {
    }
    fn release_button(
        &mut self,
        _button: u32,
        _rt: &Handle,
    ) {
    }

    fn callback(&mut self, 
        _msg: u32,
        _rt: &Handle
    )  {

    }
}

pub struct EmptyState {
    screen: Arc<Mutex<WS1in5>>
}

impl EmptyState {
    pub fn new(screen: Arc<Mutex<WS1in5>>) -> EmptyState {
        EmptyState { screen }
    }
}

impl State for EmptyState {
    fn draw(&mut self, _rt: &Handle) {
        self.screen.blocking_lock().clear_all()
            .or_log("Unable to clear screen");
    }
}

pub struct HomeState {
    coord: (usize, usize),
    layer: usize,

    focus_x: bool,

    key: Option<String>,

    resources: Arc<Resources>,
    screen: Arc<Mutex<WS1in5>>,
}

impl HomeState {
    pub fn new(resources: Arc<Resources>, screen: Arc<Mutex<WS1in5>>) -> HomeState {
        HomeState { coord: (0,0), layer: 0, focus_x: true, key: None, resources, screen }
    }
}

impl State for HomeState {
    fn enter(
        &mut self,
        _prev: StateType,
        _rt: &Handle,
    ) {
        self.coord = (0,0);
        self.key = None;
    }
    fn draw(&mut self, _rt: &Handle) {
        let mut screen = self.screen.blocking_lock();
        screen.clear_all().ok();

        match &self.key {
            Some(key) => (|| -> Result<(), ws_1in5_i2c::Error> {
                let (_, y) = screen.draw_text(0, 0, "KEY (ESC)", &self.resources.scale12, &self.resources.font, false)?;
                screen.draw_paragraph_at(0, y, &key, &self.resources.scale10, &self.resources.font, false)?;
    
                Ok(())
            })().or_log("Failed to draw to screen"),
            None => (|| -> Result<(), ws_1in5_i2c::Error> {
                let time = Utc::now();
                screen.draw_centered_text(
                    0,
                    0,
                    &time.format("%H:%M").to_string(),
                    &self.resources.scale30,
                    &self.resources.font,
                    false
                )?;
    
                screen.draw_text(
                    0, 
                    0, 
                    &format!("Lookup: {},{}:{} ", self.coord.0, self.coord.1, self.layer), 
                    &self.resources.scale10, 
                    &self.resources.font, 
                    false
                )?;
                Ok(())
            })().or_log("Failed to draw to screen"),
        };
    }

    fn hold_key(
        &mut self,
        key: u32,
        _rt: &Handle,
    ) {
        let Some(key) = char::from_u32(key as u32).and_then(|key| parse_index(key)) else {
            return;
        };

        if self.key.is_none() {
            if self.focus_x {
                self.coord = (self.coord.0 * 10 + key, self.coord.1);
            } else {
                self.coord = (self.coord.0, self.coord.1 * 10 + key);
            }
            self.screen.blocking_lock().draw_text(
                0, 
                0, 
                &format!("Lookup: {},{}:{} ", self.coord.0, self.coord.1, self.layer), 
                &self.resources.scale10, 
                &self.resources.font,
                true
            )
            .or_log("Failed to draw to screen");
        }
    }

    fn hold_special(
        &mut self,
        special: u32,
        rt: &Handle,
    ) {
        let mut lock_screen = self.screen.blocking_lock();
        let special = SpecialKey::from(special);

        match self.key {
            Some(_) => match special {
                SpecialKey::Escape => {
                    self.key = None;
                    self.focus_x = true;

                    drop(lock_screen);
                    self.draw(rt);
                },
                _ => (),
            },
            None => match special {
                SpecialKey::Escape => if !self.focus_x {
                    self.focus_x = true
                },
                SpecialKey::ReturnEnter => if self.focus_x {
                    self.focus_x = false
                } else {
                    self.key = rpc(|mut client| client.get_layer(self.layer))
                        .map_err(|e| e.to_string())
                        .and_then(|layer| get_key(self.coord, layer))
                        .or_log("RPC Failed");

                    drop(lock_screen);
                    self.draw(rt);
                },
                SpecialKey::Backspace => {
                    if self.focus_x {
                        self.coord = (self.coord.0 / 10, self.coord.1);
                    } else {
                        self.coord = (self.coord.0, self.coord.1 / 10);
                    }
                    lock_screen.draw_text(
                        0, 
                        0, 
                        &format!("Lookup: {},{}:{} ", self.coord.0, self.coord.1, self.layer), 
                        &self.resources.scale10, 
                        &self.resources.font,
                        true
                    ).or_log("Failed to draw to screen");
                }
                SpecialKey::UpArrow => {
                    self.layer += 1;
                    lock_screen.draw_text(
                        0, 
                        0, 
                        &format!("Lookup: {},{}:{} ", self.coord.0, self.coord.1, self.layer), 
                        &self.resources.scale10, 
                        &self.resources.font,
                        true
                    ).or_log("Failed to draw to screen");
                },
                SpecialKey::DownArrow => {
                    if self.layer != 0 {
                        self.layer -= 1;
                    }
                    lock_screen.draw_text(
                        0, 
                        0, 
                        &format!("Lookup: {},{}:{} ", self.coord.0, self.coord.1, self.layer), 
                        &self.resources.scale10, 
                        &self.resources.font,
                        true
                    )
                        .or_log("Failed to draw to screen");
                },
                _ => (),
            },
        }
    }
}

pub struct VariablesState {
    variables: Vec<String>,
    page: usize,
    line_size: (usize, usize),

    selected: Option<(String, String)>,
    writing_x: usize,
    writing_y: usize,

    resources: Arc<Resources>, 
    screen: Arc<Mutex<WS1in5>>
}

impl VariablesState {
    pub fn new(screen: Arc<Mutex<WS1in5>>, resources: Arc<Resources>) -> VariablesState {
        let (char_width, line_height) =
            WS1in5::size_to_pow_2(drawing::text_size(resources.scale10, &resources.font, "_"));

        let line_size = (OLED_WIDTH / char_width as usize, line_height as usize);

        VariablesState {
            variables: vec![],
            page: 0,
            line_size,
            selected: None,
            writing_x: 0,
            writing_y: 0,
            resources,
            screen
        }
    }
}

impl State for VariablesState {
    fn enter(
        &mut self,
        _prev: StateType,
        _rt: &Handle,
    ) {
        self.selected = None;
        self.variables = rpc(|mut client| client.variables()).or_log("RPC failed").unwrap_or_else(|| vec![]);
        self.page = 0;
    }

    fn draw(&mut self, _rt: &Handle) {
        let mut screen = self.screen.blocking_lock();
        screen.clear_all().ok();

        match &self.selected {
            Some((_, variable)) =>  (|| -> Result<(), ws_1in5_i2c::Error> {
                let (_, y) = screen.draw_text(
                    0, 
                    0, 
                    "EDIT (ETR: Save, ESC)", 
                    &self.resources.scale12, 
                    &self.resources.font,
                    false,
                )?;
                let xy = screen.draw_paragraph_at(
                    0, 
                    y, 
                    variable, 
                    &self.resources.scale10, 
                    &self.resources.font,
                    false
                )?;

                (self.writing_x, self.writing_y) = xy;

                Ok(())
            })().or_log("Failed to draw to screen"),
            None => (|| -> Result<(), ws_1in5_i2c::Error> {

                let (_, y) = screen.draw_text(
                    0, 
                    0, 
                    &format!("VARS {} (ETR: Save)", self.page),
                    &self.resources.scale12,
                    &self.resources.font,
                    false
                )?;

                for (i, variable) in self
                    .variables
                    .iter()
                    .skip(self.page * 9)
                    .take(9)
                    .enumerate()
                {
                    let (mut variable_image, mut width, height) = screen.create_text(
                        &format!("{}|{}", i, variable),
                        &self.resources.scale10,
                        &self.resources.font,
                        false
                    );

                    if variable_image.width() as usize > OLED_WIDTH {
                        variable_image = DynamicImage::ImageLuma8(variable_image).crop(0, 0, OLED_WIDTH as u32, height as u32).to_luma8();
                        width = OLED_WIDTH;
                    }

                    let buffer =
                        screen.get_buffer(variable_image.enumerate_pixels(), width, height)?;
                    screen.show_image(
                        buffer,
                        0,
                        y + i * self.line_size.1,
                        width,
                        height,
                    )?;
                }

                screen.draw_text(
                    0,
                    OLED_HEIGHT - 12,
                    "LEFT: <, RIGHT: >",
                    &self.resources.scale12,
                    &self.resources.font,
                    false
                )?;

                Ok(())
            })().or_log("Failed to draw to screen"),
        };
    }

    fn hold_key(
        &mut self,
        key: u32,
        rt: &Handle,
    ) {
        let Some(key) = char::from_u32(key as u32) else {
            return;
        };

        match &mut self.selected {
            Some((_, variable)) => {
                let mut screen = self.screen.blocking_lock();

                (|| -> Result<(), ws_1in5_i2c::Error> {
                    let (width, height) = screen.get_text_size("_", &self.resources.scale10, &self.resources.font);
                    (self.writing_x, _) = screen.draw_text(
                        self.writing_x, 
                        self.writing_y, 
                        &key.to_string(), 
                        &self.resources.scale10, 
                        &self.resources.font,
                        true
                    )?;

                    if self.writing_x + width > OLED_WIDTH {
                        self.writing_x = 0;
                        self.writing_y += height;
                    }
                    Ok(())
                })().or_log("Failed to draw to screen");

                variable.push(key);
            }
            None => {
                let Some(index) = parse_index(key) else {
                    return;
                };

                let Some(variable_name) = self.variables.iter().skip(self.page * 9).take(9).skip(index).next() else {
                    return;
                };

                let Some(variable) = rpc(|mut client| client.get_variable(variable_name.to_string())).or_log("RPC failed") else {
                    return;
                };

                self.selected = Some((variable_name.to_string(), variable));
                self.draw(rt);
            }
        }
    }

    fn hold_special(
        &mut self,
        special: u32,
        rt: &Handle,
    ) {
        let special = SpecialKey::from(special);

        match &mut self.selected {
            Some((_, variable)) => match special {
                SpecialKey::Escape => {
                    self.selected = None;
                    self.draw(rt);
                },
                SpecialKey::Spacebar => {
                    let (width, height) =
                    self.screen.blocking_lock().get_text_size("_", &self.resources.scale10, &self.resources.font);

                    if self.writing_x + width > OLED_WIDTH {
                        self.writing_x = 0;
                        self.writing_y += height;
                    } else {
                        self.writing_x += width;
                    }

                    variable.push(' ');
                }
                SpecialKey::ReturnEnter => {
                    if let Some((name, variable)) = self.selected.take() {
                        rpc(|mut client| client.set_variable(name, variable)).or_log("RPC failed");
                    }

                    self.draw(rt)
                }
                SpecialKey::Backspace => {
                    let mut lock_screen = self.screen.blocking_lock();

                    if self.writing_x == 0 && self.writing_y == 12 {
                        return;
                    }

                    (|| -> Result<(), ws_1in5_i2c::Error> {
                        let (width, height) =
                            lock_screen.get_text_size("_", &self.resources.scale10, &self.resources.font);
                        if self.writing_x == 0 {
                            self.writing_x = OLED_WIDTH / width * width;
                            if OLED_WIDTH % width != 0 {
                                self.writing_x -= width;
                            }
                            self.writing_y -= height;
                        } else {
                            self.writing_x -= width;
                        }
                        
                        lock_screen.clear(OLED_WIDTH - width - self.writing_x, OLED_HEIGHT - height - self.writing_y, width, height)?;
                        Ok(())
                    })().or_log("Failed to draw to screen");

                    variable.pop();
                }
                _ => return,
            },
            None => {
                if self.page != 0 && special == SpecialKey::LeftArrow {
                    self.page -= 1;
                    self.draw(rt)
                } else if special == SpecialKey::RightArrow {
                    self.page += 1;
                    self.draw(rt)
                } else if special == SpecialKey::ReturnEnter {
                    rpc(|mut client| client.save_variables()).or_log("RPC failed");
                }
            }
        }
    }
}

pub struct TermState {
    command: String,
    writing_x: usize,
    writing_y: usize,
    ctl: bool,

    line_char_num: usize,
    line_num: usize,

    process: Option<Child>,
    process_out: Option<JoinHandle<()>>,
    stdout: Arc<Mutex<String>>,
    output: Option<(String, String)>,
    page: usize,

    command_pipe: UnboundedSender<CallbackCommand>,

    resources: Arc<Resources>, 
    screen: Arc<Mutex<WS1in5>>
}

impl TermState {
    pub fn new(screen: Arc<Mutex<WS1in5>>, resources: Arc<Resources>, command_pipe: UnboundedSender<CallbackCommand>) -> TermState {
        let (_, heading_height) = screen.blocking_lock()
            .get_text_size("_", &resources.scale20, &resources.font);
        let (char_width, char_height) = screen.blocking_lock()
            .get_text_size("_", &resources.scale10, &resources.font);

        TermState {
            command: String::new(),
            writing_x: 0,
            writing_y: 0,
            ctl: false,

            process: None,
            process_out: None,
            stdout: Arc::new(Mutex::new(String::new())),
            output: None,
            
            line_char_num: OLED_WIDTH / char_width,
            line_num: (OLED_HEIGHT - heading_height) / char_height,
            page: 0,

            command_pipe,

            resources,
            screen
        }
    }
}

impl State for TermState {
    fn enter(
        &mut self,
        _prev: StateType,
        _rt: &Handle,
    ) {
        self.command = String::new();
        self.process = None;
        self.process_out = None;
        self.output = None;

        self.page = 0;
        self.writing_x = 0;
        self.writing_y = 0;
    }

    fn exit(
        &mut self,
        _next: StateType,
        _rt: &Handle,
    ) {
        if let Some(mut child) = self.process.take() {
            child.kill().ok();
        }
        self.process_out.take().map(|handle| handle.abort());
    }

    fn draw(&mut self, _rt: &Handle) {
        let mut screen = self.screen.blocking_lock();
        screen.clear_all().ok();

        match &mut self.process {
            Some(_) => (|| -> Result<(), ws_1in5_i2c::Error> {
                let (_, y) = screen.draw_centered_text(
                    0,
                    0,
                    "RUNNING",
                    &self.resources.scale12,
                    &self.resources.font,
                    false
                )?;

                screen.draw_centered_text(
                    0,
                    y,
                    "ESC: kill, ENTR: poll",
                    &self.resources.scale12,
                    &self.resources.font,
                    false
                )?;

                Ok(())
            })().or_log("Failed to draw to screen"),
            None => match &self.output {
                    Some((exit, output)) => (|| -> Result<(), ws_1in5_i2c::Error> {
                        let (_, y) = screen.draw_text(
                            0,
                            0,
                            &format!("OUTPUT ({}) (ESC)", exit),
                            &self.resources.scale12,
                            &self.resources.font,
                            false
                        )?;

                        let (mut x, mut y) = (0, y);
                        for line in output.chars()
                            .chunks(self.line_char_num)
                            .into_iter()
                            .skip(self.page * self.line_num)
                            .take(self.line_num)
                        {
                            (x, y) = screen.draw_paragraph_at(
                                x,
                                y,
                                &line.collect::<String>(),
                                &self.resources.scale10,
                                &self.resources.font,
                                false
                            )?;
                        }

                        screen.draw_paragraph_at(
                            0,
                            OLED_HEIGHT - 12,
                            "LEFT: <, RIGHT: >",
                            &self.resources.scale10,
                            &self.resources.font,
                            false
                        )?;
                        
        
                        Ok(())
                    })().or_log("Failed to draw to screen"),
                    None => (|| -> Result<(), ws_1in5_i2c::Error> {
                        let (_, y) = screen.draw_text(
                            0,
                            0,
                            "TERM (ETR: run)",
                            &self.resources.scale12,
                            &self.resources.font,
                            false
                        )?;
        
                        (self.writing_x, self.writing_y) = screen.draw_paragraph_at(
                            0,
                            y,
                            &self.command,
                            &self.resources.scale12,
                            &self.resources.font,
                            false
                        )?;
        
                        Ok(())
                    })().or_log("Failed to draw to screen"),
                },
        };
    }

    fn hold_key(
        &mut self,
        key: u32,
        rt: &Handle,
    ) {
        let Some(key) = char::from_u32(key as u32) else {
            return;
        };

        if self.ctl && key == 'c' {
            if let Some(child) = &mut self.process {
                child.kill().ok();
                self.process = None;
                self.draw(rt);
                return;
            }
        }

        if self.process.is_none() && self.output.is_none() {
            let mut screen = self.screen.blocking_lock();
            (|| -> Result<(), ws_1in5_i2c::Error> {
                let (width, height) = screen.get_text_size("_", &self.resources.scale12, &self.resources.font);
                (self.writing_x, _) = screen.draw_text(
                    self.writing_x, 
                    self.writing_y, 
                    &key.to_string(), 
                    &self.resources.scale12, 
                    &self.resources.font,
                    false
                )?;
                if self.writing_x + width > OLED_WIDTH {
                    self.writing_x = 0;
                    self.writing_y += height;
                }
                Ok(())
            })().or_log("Failed to draw to screen");
            self.command.push(key);
        }
    }

    fn hold_special(
        &mut self,
        special: u32,
        rt: &Handle,
    ) {
        let special = SpecialKey::from(special);

        match &mut self.process {
            Some(child) => match special {
                SpecialKey::Escape => {
                    child.kill().ok();
                    self.process = None;
                    self.output = None;

                    self.draw(rt);
                },
                SpecialKey::ReturnEnter => {
                    if let Ok(exit) = child.try_wait() {
                        let Some(exit) = exit else {
                            return;
                        };

                        let exit = format!("E{}", exit.code().unwrap_or(0));

                        drop(child);
                        self.process = None;
                        self.process_out.take().map(|handle| handle.abort());
                        self.output = Some((exit, self.stdout.blocking_lock().to_string()));
                        self.page = 0;
                        
                        self.draw(rt);
                    } else {
                        self.process = None;
                        self.process_out.take().map(|handle| handle.abort());
                        self.output = Some(("Failed".to_string(), "Failed to run command".to_string()));
                        self.page = 0;

                        self.draw(rt);
                    }
                },
                _ => (),
            },
            None => match &self.output {
                Some(_) => match special {
                    SpecialKey::LeftArrow => {
                        if self.page != 0 {
                            self.page -= 1;
                        } 
                        self.draw(rt);
                    },
                    SpecialKey::RightArrow => {
                        self.page += 1;
                        self.draw(rt);
                    },
                    SpecialKey::Escape => {
                        self.process = None;
                        self.output = None;
                        self.page = 0;
                        self.draw(rt);
                    }
                    _ => (),
                },
                None => match special {
                    SpecialKey::ReturnEnter => {
                        self.process = Command::new("bash")
                            .arg("-c")
                            .arg(&self.command)
                            .stdout(Stdio::piped())
                            .stderr(Stdio::piped())
                            .spawn()
                            .ok();

                        *self.stdout.blocking_lock() = String::new();
                        if let Some(child) = &mut self.process {
                            let stdout = self.stdout.clone();
                            let child_stdout = child.stdout.take().expect("Pipe was created for stdout");
                            let child_stderr = child.stderr.take().expect("Pipe was created for stderr");
                            self.process_out = Some(rt.spawn(async move {
                                let mut buf_out = BufReader::new(child_stdout).lines();
                                let mut buf_err = BufReader::new(child_stderr).lines();
                                loop {
                                    if let Some(line) = buf_out.next() {
                                        if let Ok(line) = line {
                                            let mut stdout = stdout.lock().await;
                                            stdout.push_str(&line);
                                            stdout.push('\n');
                                            drop(stdout)
                                        }
                                    }


                                    if let Some(line) = buf_err.next() {
                                        let Ok(line) = line else {
                                            break;
                                        };

                                        let mut stdout = stdout.lock().await;
                                        stdout.push_str(&line);
                                        stdout.push('\n');
                                        drop(stdout)
                                    }
                                }
                            }));
                        }
                        
                        thread::sleep(Duration::from_millis(30));
                        if let Some(child) = &mut self.process {
                            if let Ok(Some(exit)) = child.try_wait() {
                                let exit = format!("E{}", exit.code().unwrap_or(0));

                                drop(child);
                                self.process = None;
                                self.process_out.take().map(|handle| handle.abort());
                                self.output = Some((exit, self.stdout.blocking_lock().to_string()));
                                self.page = 0;
                            }
                        }
                        self.command_pipe.send(CallbackCommand { state: StateType::Term, msg: SpecialKey::Enter as u32 })
                            .or_log("Unable to start child process polling");
                        self.draw(rt);
                    },
                    SpecialKey::Spacebar => {
                        let (width, height) =
                            self.screen.blocking_lock().get_text_size("_", &self.resources.scale12, &self.resources.font);
    
                        if self.writing_x + width > OLED_WIDTH {
                            self.writing_x = 0;
                            self.writing_y += height;
                        } else {
                            self.writing_x += width;
                        }

                        self.command += " ";
                    },
                    SpecialKey::Backspace => {
                        let mut lock_screen = self.screen.blocking_lock();
                        
                        if self.writing_x == 0 && self.writing_y == 12 {
                            return;
                        }

                        (|| -> Result<(), ws_1in5_i2c::Error> {
                            let (width, height) =
                                lock_screen.get_text_size("_", &self.resources.scale12, &self.resources.font);
                            if self.writing_x == 0 {
                                self.writing_x = OLED_WIDTH / width * width;
                                if OLED_WIDTH % width != 0 {
                                    self.writing_x -= width;
                                }
                                self.writing_y -= height;
                            } else {
                                self.writing_x -= width;
                            }
                            
                            lock_screen.clear(self.writing_x, self.writing_y, width, height)?;
                            Ok(())
                        })().or_log("Failed to draw to screen");
    
                        self.command.pop();
                    },
                    _ => (),
                },
            } 
        }
    }

    fn hold_modifier(
        &mut self,
        modifier: u32,
        _rt: &Handle,
    ) {
        let modifier = Modifier::from(modifier);

        match modifier {
            Modifier::LeftControl => self.ctl = true,
            Modifier::RightControl => self.ctl = true,
            _ => (),
        }
    }

    fn release_modifier(
        &mut self,
        modifier: u32,
        _rt: &Handle,
    ) {
        let modifier = Modifier::from(modifier);

        match modifier {
            Modifier::LeftControl => self.ctl = false,
            Modifier::RightControl => self.ctl = false,
            _ => (),
        }
    }

    fn callback(&mut self, msg: u32, rt: &Handle) {
        if msg == SpecialKey::Enter as u32 {
            if let Some(child) = &mut self.process {
                if let Ok(exit) = child.try_wait() {
                    let Some(exit) = exit else {
                        return;
                    };

                    let exit = format!("E{}", exit.code().unwrap_or(0));

                    drop(child);
                    self.process = None;
                    self.process_out.take().map(|handle| handle.abort());
                    self.output = Some((exit, self.stdout.blocking_lock().to_string()));
                    self.page = 0;
                    
                    self.draw(rt);
                } else {
                    self.command_pipe.send(CallbackCommand { state: StateType::Term, msg: SpecialKey::Enter as u32 })
                        .or_log("Unable to start child process polling");
                }
            }
        }
    }
}

#[derive(Clone)]
enum ScriptState {
    Home,
    Ducky,
    Bork,
}

pub struct Scripts {
    state: ScriptState,

    ducky_scripts: Vec<(String, String)>,
    bork_scripts: Vec<(String, String)>,
    page: usize,

    line_size: (usize, usize),

    last_script: Option<(ScriptState, String, String)>,

    resources: Arc<Resources>, 
    screen: Arc<Mutex<WS1in5>>,

    system_data: Arc<SystemData>,
}

impl Scripts {
    pub fn new(system_data: Arc<SystemData>, resources: Arc<Resources>, screen: Arc<Mutex<WS1in5>>) -> Scripts {
        let (char_width, line_height) =
            WS1in5::size_to_pow_2(drawing::text_size(resources.scale10, &resources.font, "_"));

        let line_size = (OLED_WIDTH / char_width as usize, line_height as usize);

        Scripts{
            state: ScriptState::Home, 
            ducky_scripts: vec![], 
            bork_scripts: vec![], 
            page: 0, 
            line_size, 
            last_script: None, 
            resources, 
            screen,
            system_data, 
        }
    }
}

impl State for Scripts {
    fn enter(
        &mut self,
        _prev: StateType,
        _rt: &Handle,
    ) {
        self.state = ScriptState::Home;
        self.page = 0;

    }

    fn draw(&mut self, _rt: &Handle) {
        let mut screen = self.screen.blocking_lock();

        match self.state {
            ScriptState::Home => (|| -> Result<(), ws_1in5_i2c::Error> {
                let (_, y) = screen.draw_centered_text(
                    0, 
                    0, 
                    "SCRIPTS", 
                    &self.resources.scale12, 
                    &self.resources.font, 
                    false
                )?;
    
                let (_, y) = screen.draw_centered_text(
                    0, 
                    y, 
                    "ENTR: run last, D: ducky, B: bork", 
                    &self.resources.scale12, 
                    &self.resources.font, 
                    false
                )?;

                if let Some((_, name, _)) = &self.last_script {
                    let (_, _) = screen.draw_centered_text(
                        0, 
                        y, 
                        &format!("LAST: {}", name), 
                        &self.resources.scale12, 
                        &self.resources.font, 
                        false
                    )?;
                }

    
                Ok(())
            })().or_log("Failed to draw to screen"),
            ScriptState::Ducky | ScriptState::Bork => (|| -> Result<(), ws_1in5_i2c::Error> {
                let (_, y) = screen.draw_text(
                    0, 
                    0, 
                    &format!(
                        "{} {}", 
                        if matches!(self.state, ScriptState::Ducky) {
                            "DUCKY"
                        } else {
                            "BORK"
                        }, 
                        self.page
                    ),
                    &self.resources.scale12,
                    &self.resources.font,
                    false
                )?;

                for (i, (script, _)) in if matches!(self.state, ScriptState::Ducky) {
                        &self.ducky_scripts
                    } else {
                        &self.bork_scripts
                    }
                    .iter()
                    .skip(self.page * 9)
                    .take(9)
                    .enumerate()
                {
                    let (mut script_image, mut width, height) = screen.create_text(
                        &format!("{}|{}", i, script),
                        &self.resources.scale10,
                        &self.resources.font,
                        false,
                    );

                    if script_image.width() as usize > OLED_WIDTH {
                        script_image = DynamicImage::ImageLuma8(script_image).crop(0, 0, OLED_WIDTH as u32, height as u32).to_luma8();
                        width = OLED_WIDTH;
                    }

                    let buffer =
                        screen.get_buffer(script_image.enumerate_pixels(), width, height)?;
                    screen.show_image(
                        buffer,
                        0,
                        y + i * self.line_size.1,
                        width,
                        height,
                    )?;
                }

                screen.draw_text(
                    0,
                    OLED_HEIGHT - 12,
                    "LEFT: <, RIGHT: >",
                    &self.resources.scale12,
                    &self.resources.font,
                    false
                )?;

                Ok(())
            })().or_log("Failed to draw to screen"),
        };
    }

    fn hold_key(
        &mut self,
        key: u32,
        rt: &Handle,
    ) {
        let Some(key) = char::from_u32(key as u32) else {
            return;
        };

        match self.state {
            ScriptState::Home => {
                if key == 'd' {
                    self.state = ScriptState::Ducky;
                    self.page = 0;
                    self.ducky_scripts = self.system_data.ducky_scripts();
                } else if key == 'b' {
                    self.state = ScriptState::Bork;
                    self.page = 0;
                    self.ducky_scripts = self.system_data.bork_scripts();
                }
            },
            ScriptState::Ducky | ScriptState::Bork => {
                let Some(index) = parse_index(key) else {
                    return;
                };

                let Some((name, path)) = if matches!(self.state, ScriptState::Ducky) {
                    &self.ducky_scripts
                } else {
                    &self.bork_scripts
                }.iter().skip(self.page * 9).take(9).skip(index).next() else {
                    return;
                };

                let process = Command::new("bash")
                    .arg("-c")
                    .arg(format!(
                        "{} {} (SHFT+#: edit)", 
                        if matches!(self.state, ScriptState::Ducky) {
                            "quack"
                        } else {
                            "bork"
                        },
                        path
                    ))
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped())
                    .spawn()
                    .or_log("Unable to spawn script process");

                if let Some(mut child) = process {
                    rt.spawn(async move {
                        while let Ok(None) = child.try_wait() {
                            thread::sleep(Duration::from_millis(30));
                        }
                    });
                }

                self.state = ScriptState::Home;
                self.last_script = Some((self.state.clone(), name.to_string(), path.to_string()));
                self.draw(rt);
            },
        }
    }

    fn hold_special(
        &mut self,
        special: u32,
        rt: &Handle,
    ) {
        let special = SpecialKey::from(special);

        match self.state {
            ScriptState::Home => {
                if special == SpecialKey::ReturnEnter {
                    if let Some((_, _, path)) = &self.last_script {
                        let process = Command::new("bash")
                            .arg("-c")
                            .arg(format!(
                                "{} {}", 
                                if matches!(self.state, ScriptState::Ducky) {
                                    "quack"
                                } else {
                                    "bork"
                                },
                                path
                            ))
                            .stdout(Stdio::piped())
                            .stderr(Stdio::piped())
                            .spawn()
                            .or_log("Unable to spawn script process");
        
                        if let Some(mut child) = process {
                            rt.spawn(async move {
                                while let Ok(None) = child.try_wait() {
                                    thread::sleep(Duration::from_millis(30));
                                }
                            });
                        }
                    }
                }
            },
            ScriptState::Ducky | ScriptState::Bork => {
                if self.page != 0 && special == SpecialKey::LeftArrow {
                    self.page -= 1;
                    self.draw(rt)
                } else if special == SpecialKey::RightArrow {
                    self.page += 1;
                    self.draw(rt)
                } 
            },
        }
    }
}
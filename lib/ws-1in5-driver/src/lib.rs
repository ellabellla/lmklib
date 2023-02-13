use std::{
    collections::HashMap,
    io::{BufReader, BufRead},
    process::{Child, Command, Stdio},
    sync::Arc,
    time::{Duration, Instant}, thread, fmt::Display,
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
use tokio::{runtime::Runtime, sync::Mutex, task::JoinHandle};
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

pub struct ScreenHid {
    resources: Resources,
    screen: Arc<Mutex<WS1in5>>,

    shift: bool,

    rt: Runtime,
    _handle: JoinHandle<()>,

    states: HashMap<StateType, Box<dyn State>>,
    last_interact: Arc<Mutex<Instant>>,
    curr_state: StateType,
}

async fn spawn(screen: Arc<Mutex<WS1in5>>, last_interact: Arc<Mutex<Instant>>) {
    let mut last_reset = *last_interact.lock().await;
    *last_interact.lock().await = Instant::now();
    let mut interval = tokio::time::interval(Duration::from_secs(1));

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

        let resources = Resources {
            font,
            scale10,
            scale12,
            scale20,
            scale30,
        };

        let mut states: HashMap<StateType, Box<dyn State>> = HashMap::new();
        states.insert(StateType::Home, Box::new(HomeState {}));
        states.insert(
            StateType::Variables,
            Box::new(VariablesState::new(screen.clone(), &resources)),
        );
        states.insert(StateType::Term, Box::new(TermState::new(screen.clone(), &resources)));
        states.insert(StateType::Empty, Box::new(EmptyState{}));

        let last_interact = Arc::new(Mutex::new(Instant::now()));

        let rt = tokio::runtime::Runtime::new().unwrap();
        let handle = rt.spawn(spawn(screen.clone(), last_interact.clone()));

        let mut shid = ScreenHid {
            resources,
            screen,
            shift: false,
            states,
            last_interact,
            curr_state: StateType::Home,
            _handle: handle,
            rt,
        };
        shid.states.get_mut(&shid.curr_state).and_then(|state| {
            state.enter(
                shid.curr_state,
                shid.screen.clone(),
                &shid.resources,
                &shid.rt,
            );
            state.draw(shid.screen.clone(), &shid.resources, &shid.rt);
            Some(())
        });
        shid
    }

    fn change_state(&mut self, next_state: StateType) {
        if self.states.contains_key(&next_state) {
            self.states.get_mut(&self.curr_state).and_then(|state| {
                Some(state.exit(next_state, self.screen.clone(), &self.resources, &self.rt))
            });
            self.states.get_mut(&next_state).and_then(|state| {
                state.enter(
                    self.curr_state,
                    self.screen.clone(),
                    &self.resources,
                    &self.rt,
                );
                state.draw(self.screen.clone(), &self.resources, &self.rt);
                Some(())
            });
            self.curr_state = next_state;
        }
    }
}

impl HID for ScreenHid {
    fn hold_key(&mut self, mut key: usize) {
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
                key = char as usize;
            };
        }

        self.states.get_mut(&self.curr_state).and_then(|state| {
            Some(state.hold_key(key, self.screen.clone(), &self.resources, &self.rt))
        });
    }
    fn hold_special(&mut self, special: usize) {
        *self.last_interact.blocking_lock() = Instant::now();

        self.states.get_mut(&self.curr_state).and_then(|state| {
            Some(state.hold_special(special, self.screen.clone(), &self.resources, &self.rt))
        });
    }
    fn hold_modifier(&mut self, modifier: usize) {
        *self.last_interact.blocking_lock() = Instant::now();

        if modifier == Modifier::LeftShift as usize || modifier == Modifier::RightShift as usize {
            self.shift = true;
        }

        self.states.get_mut(&self.curr_state).and_then(|state| {
            Some(state.hold_modifier(modifier, self.screen.clone(), &self.resources, &self.rt))
        });
    }

    fn release_key(&mut self, key: usize) {
        *self.last_interact.blocking_lock() = Instant::now();

        self.states.get_mut(&self.curr_state).and_then(|state| {
            Some(state.release_key(key, self.screen.clone(), &self.resources, &self.rt))
        });
    }
    fn release_special(&mut self, special: usize) {
        self.states.get_mut(&self.curr_state).and_then(|state| {
            Some(state.release_special(special, self.screen.clone(), &self.resources, &self.rt))
        });
    }
    fn release_modifier(&mut self, modifier: usize) {
        *self.last_interact.blocking_lock() = Instant::now();

        if modifier == Modifier::LeftShift as usize || modifier == Modifier::RightShift as usize {
            self.shift = false;
        }

        self.states.get_mut(&self.curr_state).and_then(|state| {
            Some(state.release_modifier(modifier, self.screen.clone(), &self.resources, &self.rt))
        });
    }

    fn press_basic_str(&mut self, str: RString) {
        *self.last_interact.blocking_lock() = Instant::now();

        self.states.get_mut(&self.curr_state).and_then(|state| {
            Some(state.press_basic_str(str, self.screen.clone(), &self.resources, &self.rt))
        });
    }

    fn press_str(&mut self, _: RString, str: RString) {
        self.press_basic_str(str)
    }

    fn scroll_wheel(&mut self, amount: i8) {
        *self.last_interact.blocking_lock() = Instant::now();

        self.states.get_mut(&self.curr_state).and_then(|state| {
            Some(state.scroll_wheel(amount, self.screen.clone(), &self.resources, &self.rt))
        });
    }

    fn move_mouse_x(&mut self, amount: i8) {
        *self.last_interact.blocking_lock() = Instant::now();

        self.states.get_mut(&self.curr_state).and_then(|state| {
            Some(state.move_mouse_x(amount, self.screen.clone(), &self.resources, &self.rt))
        });
    }
    fn move_mouse_y(&mut self, amount: i8) {
        *self.last_interact.blocking_lock() = Instant::now();

        self.states.get_mut(&self.curr_state).and_then(|state| {
            Some(state.move_mouse_y(amount, self.screen.clone(), &self.resources, &self.rt))
        });
    }

    fn hold_button(&mut self, button: usize) {
        *self.last_interact.blocking_lock() = Instant::now();

        self.states.get_mut(&self.curr_state).and_then(|state| {
            Some(state.hold_button(button, self.screen.clone(), &self.resources, &self.rt))
        });
    }
    fn release_button(&mut self, button: usize) {
        *self.last_interact.blocking_lock() = Instant::now();

        self.states.get_mut(&self.curr_state).and_then(|state| {
            Some(state.release_button(button, self.screen.clone(), &self.resources, &self.rt))
        });
    }

    fn send_keyboard(&mut self) {}

    fn send_mouse(&mut self) {}

    fn send_command(&mut self, data: RString) {
        *self.last_interact.blocking_lock() = Instant::now();
        match data.as_str() {
            "wake" => {
                if self.curr_state != StateType::Empty {
                    self.states.get_mut(&self.curr_state).and_then(|state| {
                        Some(state.draw(self.screen.clone(), &self.resources, &self.rt))
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

pub trait State {
    fn draw(&mut self, screen: Arc<Mutex<WS1in5>>, _resources: &Resources, _rt: &Runtime) {
        screen.blocking_lock().clear_all().ok();
    }

    fn enter(
        &mut self,
        _prev: StateType,
        _screen: Arc<Mutex<WS1in5>>,
        _resources: &Resources,
        _rt: &Runtime,
    ) {
    }
    fn exit(
        &mut self,
        _next: StateType,
        _screen: Arc<Mutex<WS1in5>>,
        _resources: &Resources,
        _rt: &Runtime,
    ) {
    }

    fn hold_key(
        &mut self,
        _key: usize,
        _screen: Arc<Mutex<WS1in5>>,
        _resources: &Resources,
        _rt: &Runtime,
    ) {
    }
    fn hold_special(
        &mut self,
        _special: usize,
        _screen: Arc<Mutex<WS1in5>>,
        _resources: &Resources,
        _rt: &Runtime,
    ) {
    }
    fn hold_modifier(
        &mut self,
        _modifier: usize,
        _screen: Arc<Mutex<WS1in5>>,
        _resources: &Resources,
        _rt: &Runtime,
    ) {
    }
    fn release_key(
        &mut self,
        _key: usize,
        _screen: Arc<Mutex<WS1in5>>,
        _resources: &Resources,
        _rt: &Runtime,
    ) {
    }
    fn release_special(
        &mut self,
        _special: usize,
        _screen: Arc<Mutex<WS1in5>>,
        _resources: &Resources,
        _rt: &Runtime,
    ) {
    }
    fn release_modifier(
        &mut self,
        _modifier: usize,
        _screen: Arc<Mutex<WS1in5>>,
        _resources: &Resources,
        _rt: &Runtime,
    ) {
    }
    fn press_basic_str(
        &mut self,
        _str: RString,
        _screen: Arc<Mutex<WS1in5>>,
        _resources: &Resources,
        _rt: &Runtime,
    ) {
    }
    fn scroll_wheel(
        &mut self,
        _amount: i8,
        _screen: Arc<Mutex<WS1in5>>,
        _resources: &Resources,
        _rt: &Runtime,
    ) {
    }
    fn move_mouse_x(
        &mut self,
        _amount: i8,
        _screen: Arc<Mutex<WS1in5>>,
        _resources: &Resources,
        _rt: &Runtime,
    ) {
    }
    fn move_mouse_y(
        &mut self,
        _amount: i8,
        _screen: Arc<Mutex<WS1in5>>,
        _resources: &Resources,
        _rt: &Runtime,
    ) {
    }
    fn hold_button(
        &mut self,
        _button: usize,
        _screen: Arc<Mutex<WS1in5>>,
        _resources: &Resources,
        _rt: &Runtime,
    ) {
    }
    fn release_button(
        &mut self,
        _button: usize,
        _screen: Arc<Mutex<WS1in5>>,
        _resources: &Resources,
        _rt: &Runtime,
    ) {
    }
}

pub struct EmptyState{}
impl State for EmptyState {
    
}

pub struct HomeState {}
impl State for HomeState {
    fn draw(&mut self, screen: Arc<Mutex<WS1in5>>, resources: &Resources, _rt: &Runtime) {
        let mut screen = screen.blocking_lock();
        screen.clear_all().ok();

        (|| -> Result<(), ws_1in5_i2c::Error> {
            let time = Utc::now();
            let (time_image, width, height) = screen.create_text(
                &time.format("%H:%M").to_string(),
                &resources.scale30,
                &resources.font,
            );
            let buffer = screen.get_buffer(time_image.enumerate_pixels(), width, height)?;
            screen.show_image(
                buffer,
                OLED_WIDTH - (OLED_WIDTH / 2 + width / 2),
                OLED_HEIGHT - height - (OLED_HEIGHT / 2 - height / 2),
                width,
                height,
            )
        })().or_log("Failed to draw to screen");
    }
}

pub struct VariablesState {
    variables: Vec<String>,
    page: usize,
    line_size: (usize, usize),

    selected: Option<(String, String)>,
    writing_x: usize,
    writing_y: usize,
}

impl VariablesState {
    pub fn new(_screen: Arc<Mutex<WS1in5>>, resources: &Resources) -> VariablesState {
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
        }
    }
}

impl State for VariablesState {
    fn enter(
        &mut self,
        _prev: StateType,
        _screen: Arc<Mutex<WS1in5>>,
        _resources: &Resources,
        _rt: &Runtime,
    ) {
        self.selected = None;
        self.variables = rpc(|mut client| client.variables()).or_log("RPC failed").unwrap_or_else(|| vec![]);
        self.page = 0;
    }

    fn draw(&mut self, screen: Arc<Mutex<WS1in5>>, resources: &Resources, _rt: &Runtime) {
        let mut screen = screen.blocking_lock();
        screen.clear_all().ok();

        match &self.selected {
            Some((_, variable)) =>  (|| -> Result<(), ws_1in5_i2c::Error> {
                let (_, y) = screen.draw_text(0, 0, "EDIT (ETR: Save, ESC)", &resources.scale12, &resources.font)?;
                let xy = screen.draw_paragraph_at(0, y, variable, &resources.scale10, &resources.font)?;

                (self.writing_x, self.writing_y) = xy;

                Ok(())
            })().or_log("Failed to draw to screen"),
            None => (|| -> Result<(), ws_1in5_i2c::Error> {

                let (_, y) = screen.draw_text(
                    0, 
                    0, 
                    &format!("VARS {} (ETR: Save)", self.page),
                    &resources.scale12,
                    &resources.font
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
                        &resources.scale10,
                        &resources.font,
                    );

                    if variable_image.width() as usize > OLED_WIDTH {
                        variable_image = DynamicImage::ImageLuma8(variable_image).crop(0, 0, OLED_WIDTH as u32, height as u32).to_luma8();
                        width = OLED_WIDTH;
                    }

                    let buffer =
                        screen.get_buffer(variable_image.enumerate_pixels(), width, height)?;
                    screen.show_image(
                        buffer,
                        OLED_WIDTH - width,
                        OLED_HEIGHT - height - (y + i * self.line_size.1),
                        width,
                        height,
                    )?;
                }

                screen.draw_text(
                    0,
                    OLED_HEIGHT - 12,
                    "LEFT: <, RIGHT: >",
                    &resources.scale12,
                    &resources.font,
                )?;

                Ok(())
            })().or_log("Failed to draw to screen"),
        };
    }

    fn hold_key(
        &mut self,
        key: usize,
        screen: Arc<Mutex<WS1in5>>,
        resources: &Resources,
        rt: &Runtime,
    ) {
        let Some(key) = char::from_u32(key as u32) else {
            return;
        };

        match &mut self.selected {
            Some((_, variable)) => {
                let mut screen = screen.blocking_lock();

                (|| -> Result<(), ws_1in5_i2c::Error> {
                    let (width, height) = screen.get_text_size("_", &resources.scale10, &resources.font);
                    (self.writing_x, _) = screen.draw_text(self.writing_x, self.writing_y, &key.to_string(), &resources.scale10, &resources.font)?;
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
                self.draw(screen, resources, rt);
            }
        }
    }

    fn hold_special(
        &mut self,
        special: usize,
        screen: Arc<Mutex<WS1in5>>,
        resources: &Resources,
        rt: &Runtime,
    ) {
        let special = SpecialKey::from(special);

        match &mut self.selected {
            Some((_, variable)) => match special {
                SpecialKey::Escape => {
                    self.selected = None;
                    self.draw(screen, resources, rt);
                },
                SpecialKey::Spacebar => {
                    let (width, height) =
                    screen.blocking_lock().get_text_size("_", &resources.scale10, &resources.font);

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

                    self.draw(screen, resources, rt)
                }
                SpecialKey::Backspace => {
                    let mut lock_screen = screen.blocking_lock();

                    if self.writing_x == 0 && self.writing_y == 12 {
                        return;
                    }

                    (|| -> Result<(), ws_1in5_i2c::Error> {
                        let (width, height) =
                            lock_screen.get_text_size("_", &resources.scale10, &resources.font);
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
                    self.draw(screen, resources, rt)
                } else if special == SpecialKey::RightArrow {
                    self.page += 1;
                    self.draw(screen, resources, rt)
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
}

impl TermState {
    pub fn new(screen: Arc<Mutex<WS1in5>>, resources: &Resources) -> TermState {
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
        }
    }
}

impl State for TermState {
    fn enter(
        &mut self,
        _prev: StateType,
        _screen: Arc<Mutex<WS1in5>>,
        _resources: &Resources,
        _rt: &Runtime,
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
        _screen: Arc<Mutex<WS1in5>>,
        _resources: &Resources,
        _rt: &Runtime,
    ) {
        if let Some(mut child) = self.process.take() {
            child.kill().ok();
        }
        self.process_out.take().map(|handle| handle.abort());
    }

    fn draw(&mut self, screen: Arc<Mutex<WS1in5>>, resources: &Resources, _rt: &Runtime) {
        let mut screen = screen.blocking_lock();
        screen.clear_all().ok();

        match &mut self.process {
            Some(_) => (|| -> Result<(), ws_1in5_i2c::Error> {
                let (_, y) = screen.draw_centered_text(
                    0,
                    0,
                    "RUNNING",
                    &resources.scale12,
                    &resources.font,
                )?;

                screen.draw_centered_text(
                    0,
                    y,
                    "ESC: kill, ENTR: poll",
                    &resources.scale12,
                    &resources.font,
                )?;

                Ok(())
            })().or_log("Failed to draw to screen"),
            None => match &self.output {
                    Some((exit, output)) => (|| -> Result<(), ws_1in5_i2c::Error> {
                        let (_, y) = screen.draw_text(
                            0,
                            0,
                            &format!("OUTPUT ({}) (ESC)", exit),
                            &resources.scale12,
                            &resources.font,
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
                                &resources.scale10,
                                &resources.font,
                            )?;
                        }

                        screen.draw_paragraph_at(
                            0,
                            OLED_HEIGHT - 12,
                            "LEFT: <, RIGHT: >",
                            &resources.scale10,
                            &resources.font,
                        )?;
                        
        
                        Ok(())
                    })().or_log("Failed to draw to screen"),
                    None => (|| -> Result<(), ws_1in5_i2c::Error> {
                        let (_, y) = screen.draw_text(
                            0,
                            0,
                            "TERM (ETR: run)",
                            &resources.scale12,
                            &resources.font,
                        )?;
        
                        (self.writing_x, self.writing_y) = screen.draw_paragraph_at(
                            0,
                            y,
                            &self.command,
                            &resources.scale12,
                            &resources.font,
                        )?;
        
                        Ok(())
                    })().or_log("Failed to draw to screen"),
                },
        };
    }

    fn hold_key(
        &mut self,
        key: usize,
        screen: Arc<Mutex<WS1in5>>,
        resources: &Resources,
        rt: &Runtime,
    ) {
        let Some(key) = char::from_u32(key as u32) else {
            return;
        };

        if self.ctl && key == 'c' {
            if let Some(child) = &mut self.process {
                child.kill().ok();
                self.process = None;
                self.draw(screen, resources, rt);
                return;
            }
        }

        if self.process.is_none() && self.output.is_none() {
            let mut screen = screen.blocking_lock();
            (|| -> Result<(), ws_1in5_i2c::Error> {
                let (width, height) = screen.get_text_size("_", &resources.scale12, &resources.font);
                (self.writing_x, _) = screen.draw_text(self.writing_x, self.writing_y, &key.to_string(), &resources.scale12, &resources.font)?;
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
        special: usize,
        screen: Arc<Mutex<WS1in5>>,
        resources: &Resources,
        rt: &Runtime,
    ) {
        let special = SpecialKey::from(special);

        match &mut self.process {
            Some(child) => match special {
                SpecialKey::Escape => {
                    child.kill().ok();
                    self.process = None;
                    self.output = None;

                    self.draw(screen, resources, rt);
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
                        
                        self.draw(screen, resources, rt);
                    } else {
                        self.process = None;
                        self.process_out.take().map(|handle| handle.abort());
                        self.output = Some(("Failed".to_string(), "Failed to run command".to_string()));
                        self.page = 0;

                        self.draw(screen, resources, rt);
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
                        self.draw(screen, resources, rt);
                    },
                    SpecialKey::RightArrow => {
                        self.page += 1;
                        self.draw(screen, resources, rt);
                    },
                    SpecialKey::Escape => {
                        self.process = None;
                        self.output = None;
                        self.page = 0;
                        self.draw(screen, resources, rt);
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
                        self.draw(screen, resources, rt);
                    },
                    SpecialKey::Spacebar => {
                        let (width, height) =
                                screen.blocking_lock().get_text_size("_", &resources.scale12, &resources.font);
    
                        if self.writing_x + width > OLED_WIDTH {
                            self.writing_x = 0;
                            self.writing_y += height;
                        } else {
                            self.writing_x += width;
                        }

                        self.command += " ";
                    },
                    SpecialKey::Backspace => {
                        let mut lock_screen = screen.blocking_lock();
                        
                        if self.writing_x == 0 && self.writing_y == 12 {
                            return;
                        }

                        (|| -> Result<(), ws_1in5_i2c::Error> {
                            let (width, height) =
                                lock_screen.get_text_size("_", &resources.scale12, &resources.font);
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
    
                        self.command.pop();
                    },
                    _ => (),
                },
            } 
        }
    }

    fn hold_modifier(
        &mut self,
        modifier: usize,
        _screen: Arc<Mutex<WS1in5>>,
        _resources: &Resources,
        _rt: &Runtime,
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
        modifier: usize,
        _screen: Arc<Mutex<WS1in5>>,
        _resources: &Resources,
        _rt: &Runtime,
    ) {
        let modifier = Modifier::from(modifier);

        match modifier {
            Modifier::LeftControl => self.ctl = false,
            Modifier::RightControl => self.ctl = false,
            _ => (),
        }
    }
}

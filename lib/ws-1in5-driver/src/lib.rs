use std::{collections::HashMap, time::{Instant, Duration}, sync::{Arc}};

use abi_stable::{export_root_module, prefix_type::PrefixTypeTrait, sabi_extern_fn, sabi_trait::TD_Opaque, std_types::RString};
use imageproc::drawing;
use key_module::hid::{HID, HidModuleRef, HidModule, HIDBox};
use key_rpc::Client;
use rusttype::{Font, Scale};
use chrono::Utc;
use tokio::{runtime::{Runtime, Handle}, sync::Mutex, task::JoinHandle};
use ws_1in5_i2c::{WS1in5, OLED_WIDTH, OLED_HEIGHT};

#[export_root_module]
pub fn get_library() -> HidModuleRef {
    HidModule{
        new_hid
    }.leak_into_prefix()
}

#[sabi_extern_fn]
fn new_hid() -> HIDBox {
    HIDBox::from_value(ScreenHid::new(), TD_Opaque)
}

pub struct Resources {
    pub font: Font<'static>,
    pub scale10: Scale,
    pub  scale12: Scale,
    pub scale20: Scale,
    pub scale30: Scale,
}

pub struct ScreenHid {
    resources: Resources,
    screen: Arc<Mutex<WS1in5>>,

    rt: Runtime,
    handle: JoinHandle<()>,

    states: HashMap<String, Box<dyn State>>,
    last_interact: Arc<Mutex<Instant>>,
    curr_state: String,
}

async fn spawn(screen: Arc<Mutex<WS1in5>>, last_interact: Arc<Mutex<Instant>>) {
    let mut last_reset = *last_interact.lock().await;
    *last_interact.lock().await = Instant::now();
    let mut interval = tokio::time::interval(Duration::from_secs(1)); 

    loop {
        interval.tick().await;
        let lock_last_interact = last_interact.lock().await;
        if last_reset != *lock_last_interact && Instant::now() - *lock_last_interact >= Duration::from_secs(10) {
            let screen = screen.clone();
            tokio::task::spawn_blocking(move || {
                screen.blocking_lock().clear_all().ok();
            }).await.ok();
            last_reset = *lock_last_interact;
        }
        drop(lock_last_interact);

    }
}

impl ScreenHid {
    pub fn new() -> ScreenHid {
        let screen = Arc::new(Mutex::new(WS1in5::new(0x3c, 3, 22)
            .and_then(|screen|screen.clear_all().map(|_| screen)).expect("Screen initialization")));

        let font_data: &[u8] = include_bytes!("../font/NotoSansMono-VariableFont_wdth,wght.ttf");
        let font: Font<'static> = Font::try_from_bytes(font_data).expect("Valid font");
        
        let scale10 = Scale::uniform(10.0);

        let scale12 = Scale::uniform(12.0);

        let scale20 = Scale::uniform(20.0);

        let scale30 = Scale::uniform(30.0);
        
        let resources = Resources{font, scale10, scale12, scale20, scale30};

        let mut states: HashMap<String, Box<dyn State>> = HashMap::new();
        states.insert("home".to_string(), Box::new(HomeState{}));
        states.insert("variables".to_string(), Box::new(VariablesState::new(screen.clone(), &resources)));
        
        let last_interact = Arc::new(Mutex::new(Instant::now()));

        let rt = tokio::runtime::Runtime::new().unwrap();
        let handle = rt.spawn(spawn(screen.clone(), last_interact.clone()));
        
        let mut shid = ScreenHid{resources, screen, states, last_interact, curr_state: "home".to_string(), handle, rt };
        shid.states.get_mut(&shid.curr_state)
            .and_then(|state| {
                state.enter(&shid.curr_state, shid.screen.clone(), &shid.resources);
                state.draw(shid.screen.clone(), &shid.resources);
                Some(())
            });
        shid
    }

    fn change_state(&mut self, name: &str) {
        if self.states.contains_key(name) {
            self.states.get_mut(&self.curr_state)
                .and_then(|state| Some(state.exit(name, self.screen.clone(), &self.resources)));
            self.states.get_mut(name)
                .and_then(|state| {
                    state.enter(&self.curr_state, self.screen.clone(), &self.resources);
                    state.draw(self.screen.clone(), &self.resources);
                    Some(())
                });
            self.curr_state = name.to_string();
        }
    }
}

impl HID for ScreenHid {
    fn hold_key(&mut self, key: usize) {
        *self.last_interact.blocking_lock() = Instant::now();

        self.states.get_mut(&self.curr_state)
            .and_then(|state| Some(state.hold_key(key, self.screen.clone(), &self.resources)));
    }
    fn hold_special(&mut self, special: usize) {
        *self.last_interact.blocking_lock() = Instant::now();

        self.states.get_mut(&self.curr_state)
            .and_then(|state| Some(state.hold_special(special, self.screen.clone(), &self.resources)));
    }
    fn hold_modifier(&mut self, modifier: usize) {
        *self.last_interact.blocking_lock() = Instant::now();

        self.states.get_mut(&self.curr_state)
            .and_then(|state| Some(state.hold_modifier(modifier, self.screen.clone(), &self.resources)));
    }

    fn release_key(&mut self, key: usize) {
        *self.last_interact.blocking_lock() = Instant::now();

        self.states.get_mut(&self.curr_state)
            .and_then(|state| Some(state.release_key(key, self.screen.clone(), &self.resources)));
    }
    fn release_special(&mut self, special: usize) {
        self.states.get_mut(&self.curr_state)
            .and_then(|state| Some(state.release_special(special, self.screen.clone(), &self.resources)));
    }
    fn release_modifier(&mut self, modifier: usize) {
        *self.last_interact.blocking_lock() = Instant::now();

        self.states.get_mut(&self.curr_state)
            .and_then(|state| Some(state.release_modifier(modifier, self.screen.clone(), &self.resources)));
    }

    fn press_basic_str(&mut self, str: RString) {
        *self.last_interact.blocking_lock() = Instant::now();

        self.states.get_mut(&self.curr_state)
            .and_then(|state| Some(state.press_basic_str(str, self.screen.clone(), &self.resources)));
    }

    fn press_str(&mut self, _: RString, str: RString) {
        self.press_basic_str(str)
    }
    
    fn scroll_wheel(&mut self, amount: i8) {
        *self.last_interact.blocking_lock() = Instant::now();
        
        self.states.get_mut(&self.curr_state)
            .and_then(|state| Some(state.scroll_wheel(amount, self.screen.clone(), &self.resources)));
    }

    fn move_mouse_x(&mut self, amount: i8) {
        *self.last_interact.blocking_lock() = Instant::now();

        self.states.get_mut(&self.curr_state)
            .and_then(|state| Some(state.move_mouse_x(amount, self.screen.clone(), &self.resources)));
    }
    fn move_mouse_y(&mut self, amount: i8) {
        *self.last_interact.blocking_lock() = Instant::now();

        self.states.get_mut(&self.curr_state)
            .and_then(|state| Some(state.move_mouse_y(amount, self.screen.clone(), &self.resources)));
    }

    fn hold_button(&mut self, button: usize) {
        *self.last_interact.blocking_lock() = Instant::now();

        self.states.get_mut(&self.curr_state)
            .and_then(|state| Some(state.hold_button(button, self.screen.clone(), &self.resources)));
    }
    fn release_button(&mut self, button: usize) {
        *self.last_interact.blocking_lock() = Instant::now();

        self.states.get_mut(&self.curr_state)
            .and_then(|state| Some(state.release_button(button, self.screen.clone(), &self.resources)));
    }

    fn send_keyboard(&mut self) { 
        
    }

    fn send_mouse(&mut self) { 
        
    }

    fn send_command(&mut self, data: RString) {
        *self.last_interact.blocking_lock() = Instant::now();
        match data.as_str() {
            "wake" => {
                self.states.get_mut(&self.curr_state)
                    .and_then(|state| Some(state.draw(self.screen.clone(), &self.resources)));
            },
            "variables" => {
                self.change_state("variables");
            },
            "exit" => {
                self.change_state("home");
            }
            _ => {}
        }
    }
}

pub trait State {
    fn draw(&mut self, screen: Arc<Mutex<WS1in5>>, resources: &Resources) {
        screen.blocking_lock().clear_all().ok();
    }

    fn enter(&mut self, prev: &str, screen: Arc<Mutex<WS1in5>>, resources: &Resources) {}
    fn exit(&mut self, next: &str, screen: Arc<Mutex<WS1in5>>, resources: &Resources) {}

    fn hold_key(&mut self, key: usize, screen: Arc<Mutex<WS1in5>>, resources: &Resources) {}
    fn hold_special(&mut self, special: usize, screen: Arc<Mutex<WS1in5>>, resources: &Resources) {}
    fn hold_modifier(&mut self, modifier: usize, screen: Arc<Mutex<WS1in5>>, resources: &Resources) {}
    fn release_key(&mut self, key: usize, screen: Arc<Mutex<WS1in5>>, resources: &Resources) {}
    fn release_special(&mut self, special: usize, screen: Arc<Mutex<WS1in5>>, resources: &Resources) {}
    fn release_modifier(&mut self, modifier: usize, screen: Arc<Mutex<WS1in5>>, resources: &Resources) {}
    fn press_basic_str(&mut self, str: RString, screen: Arc<Mutex<WS1in5>>, resources: &Resources) {}
    fn scroll_wheel(&mut self, amount: i8, screen: Arc<Mutex<WS1in5>>, resources: &Resources) {}
    fn move_mouse_x(&mut self, amount: i8, screen: Arc<Mutex<WS1in5>>, resources: &Resources) {}
    fn move_mouse_y(&mut self, amount: i8, screen: Arc<Mutex<WS1in5>>, resources: &Resources) {}
    fn hold_button(&mut self, button: usize, screen: Arc<Mutex<WS1in5>>, resources: &Resources) {}
    fn release_button(&mut self, button: usize, screen: Arc<Mutex<WS1in5>>, resources: &Resources) {}
}

pub struct HomeState{}
impl State for HomeState {
    fn draw(&mut self, screen: Arc<Mutex<WS1in5>>, resources: &Resources) {
        let screen = screen.blocking_lock();
        screen.clear_all().ok();
        
        println!("{:?}", (|| -> Result<(), ws_1in5_i2c::Error> {
            let time = Utc::now();
            let (time_image, width, height) = screen.create_text(&time.format("%H:%M").to_string(), &resources.scale30, &resources.font);
            let buffer = screen.get_buffer(time_image.enumerate_pixels(), width, height)?;
            screen.show_image(buffer, OLED_WIDTH - (OLED_WIDTH/2 + width/2), OLED_HEIGHT - height - (OLED_HEIGHT/2 - height/2), width, height)
        })());
    }
}

pub struct VariablesState{
    variables: Vec<String>,
    page: usize,
    line_size: (usize, usize),
    start: usize,
}

impl VariablesState {
    pub fn new(screen: Arc<Mutex<WS1in5>>, resources: &Resources) -> VariablesState {
        let head_char_size = WS1in5::size_to_pow_2(drawing::text_size(resources.scale12, &resources.font, "_"));
        let start = head_char_size.1 as usize;
        let (char_width, line_height) = WS1in5::size_to_pow_2(drawing::text_size(resources.scale10, &resources.font, "_"));

        let line_size = (OLED_WIDTH / char_width as usize, line_height as usize);

        VariablesState{variables: vec![], page: 0, line_size, start}
    }
}

impl State for VariablesState {
    fn enter(&mut self, prev: &str, screen: Arc<Mutex<WS1in5>>, resources: &Resources) {
        self.variables = Client::new("ipc:///lmk/ksf.ipc").ok() 
            .and_then(|mut client| client.variables().ok()).unwrap_or_else(|| vec![]);

        self.page = 0;
    }

    fn draw(&mut self, screen: Arc<Mutex<WS1in5>>, resources: &Resources) {
        let screen = screen.blocking_lock();
        screen.clear_all().ok();
        
        println!("{:?}", (|| -> Result<(), ws_1in5_i2c::Error> {
            let (header_image, width, height) = screen.create_text(&format!("VARIABLES ({})", self.page), &resources.scale12, &resources.font);
            let buffer = screen.get_buffer(header_image.enumerate_pixels(), width, height)?;
            screen.show_image(buffer, OLED_WIDTH - width, OLED_HEIGHT - height, width, height)?;

            for (i, variable) in self.variables.iter().skip(self.page * 9).take(9).enumerate() {
                let (variable_image, width, height) = screen.create_text(&format!("i|{}", variable), &resources.scale10, &resources.font);
                let buffer = screen.get_buffer(variable_image.enumerate_pixels(), width, height)?;
                screen.show_image(buffer, OLED_WIDTH - width, OLED_HEIGHT - height - (self.start + i * self.line_size.1), width, height)?;
            }
            
            let (footer_image, width, height) = screen.create_text("LEFT: prev, RIGHT: next", &resources.scale12, &resources.font);
            let buffer = screen.get_buffer(footer_image.enumerate_pixels(), width, height)?;
            screen.show_image(buffer, OLED_WIDTH - width, 0, width, height)?;

            Ok(())
        })());
    }

    fn hold_special(&mut self, special: usize, screen: Arc<Mutex<WS1in5>>, resources: &Resources) {
        if self.page != 0 && special == 13 {
            self.page -= 1;
            self.draw(screen, resources)
        } else if special == 14 {
            self.page += 1;
            self.draw(screen, resources)
        }
    }
}
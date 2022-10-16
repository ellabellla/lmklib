use std::{collections::HashMap, time::Duration, thread};

use lmk_hid::{key::{Keyboard, Key}, HID};

use crate::parser::{parse_define, parse_line};


struct QuackInterp {
    consts: HashMap<String, String>,
    variables: HashMap<String, i64>,
    lines:Vec<String>,
    keyboard: Keyboard,
    hid: HID,
}

impl QuackInterp {
    pub fn new(script: &str, hid: HID) -> QuackInterp {
        let line_itr = script.lines();
        let mut lines = Vec::new();
        let mut consts = HashMap::new();
        for line in line_itr {
            if let Ok((_, (name, text))) = parse_define(line) {
                consts.insert(name.to_string(), text.to_string());
            } else {
                lines.push(line.to_string())
            }
        }
        for line in &mut lines {
            for (word, text) in &consts {
                *line = line.replace(&format!(" {} ", word), text);
                *line = line.replace(&format!(" {}\n", word), text);
            } 
        }
        QuackInterp { consts, variables: HashMap::new(), lines, hid, keyboard: Keyboard::new() }
    }

    pub fn interpret(&mut self, line: &str) {
        let command = match parse_line(line) {
            Ok((_, command)) => command,
            Err(_) => return,
        };

        match command {
            crate::parser::Command::Rem => (),
            crate::parser::Command::String(str) => self.keyboard.press_string(str),
            crate::parser::Command::StringLN(str) => self.keyboard.press_string(str),
            crate::parser::Command::Special(special) => {self.keyboard.press_key(&Key::Special(special));},
            crate::parser::Command::Modifier(modifier) => self.keyboard.press_modifier(&modifier),
            crate::parser::Command::Shortcut(modifiers, key) => {self.keyboard.press_shortcut(&modifiers, &key);},
            crate::parser::Command::Delay(mut amount) => {
                if amount < 20 {
                    amount = 20;
                }
                
                thread::sleep(Duration::from_millis(amount));
            },
        };
    }
}
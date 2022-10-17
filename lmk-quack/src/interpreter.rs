use std::{collections::HashMap, time::Duration, thread};

use lmk_hid::{key::{Keyboard, Key}, HID};

use crate::parser::{parse_define, parse_line};


pub struct QuackInterp {
    lines:Vec<String>,
}

impl QuackInterp {
    pub fn new(script: &str) -> QuackInterp {
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
            line.push('\n');
            for (word, text) in &consts {
                *line = line.replace(&format!(" {} ", word), text);
                *line = line.replace(&format!(" {}\n", word), text);
            } 
        }
        QuackInterp { lines }
    }

    fn interpret(&self, line: &str, keyboard: &mut Keyboard, variables: &mut HashMap<String, i64>) {
        let command = match parse_line(line) {
            Ok((_, command)) => command,
            Err(_) => return,
        };

        match command {
            crate::parser::Command::Rem(comment) => println!("{}", comment),
            crate::parser::Command::String(str) => keyboard.press_string(str),
            crate::parser::Command::StringLN(str) => keyboard.press_string(str),
            crate::parser::Command::Special(special) => {keyboard.press_key(&Key::Special(special));},
            crate::parser::Command::Modifier(modifier) => keyboard.press_modifier(&modifier),
            crate::parser::Command::Shortcut(modifiers, key) => {keyboard.press_shortcut(&modifiers, &key);},
            crate::parser::Command::Delay(amount) => {
                match amount {
                    crate::parser::Value::Int(int) => thread::sleep(Duration::from_millis(int)),
                    crate::parser::Value::Variable(name) => match variables.get(name) {
                        Some(value) => thread::sleep(Duration::from_millis(u64::try_from(*value).unwrap_or(0))),
                        None => return,
                    },
                }
            },
            crate::parser::Command::Hold(key) => {keyboard.hold(&key);},
            crate::parser::Command::Release(key) => keyboard.release(&key),
            crate::parser::Command::HoldMod(modifier) => keyboard.hold_mod(&modifier),
            crate::parser::Command::ReleaseMod(modifier) => keyboard.release_mod(&modifier),
            crate::parser::Command::InjectMod => (),
        };
    }

    pub fn run(&self, hid: &mut HID) {
        let mut keyboard = Keyboard::new();
        let mut variables = HashMap::new();
        for line in &self.lines {
            self.interpret(line, &mut keyboard, &mut variables);
            keyboard.send(hid).unwrap();
        }
    }
}

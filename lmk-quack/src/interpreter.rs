use std::{collections::HashMap, time::Duration, thread};

use lmk_hid::{key::{Keyboard, Key, SpecialKey}, HID};

use crate::parser::{parse_define, parse_line, Value, Expression, Operator, Command, parse_function};


pub struct QuackInterp {
    lines:Vec<String>,
    functions: HashMap<String, usize>,
}

impl QuackInterp {
    pub fn new(script: &str) -> QuackInterp {
        let line_itr = script.lines();
        let mut lines = Vec::new();
        let mut consts = HashMap::new();
        let mut functions = HashMap::new();
        for line in line_itr {
            if let Ok((_, (name, text))) = parse_define(line) {
                consts.insert(name.to_string(), text.to_string());
            } else {
                lines.push(line.to_string())
            }
        }
        let mut i = 0;
        for line in &mut lines {
            line.push('\n');
            for (word, text) in &consts {
                *line = line.replace(&format!(" {} ", word), text);
                *line = line.replace(&format!(" {}\n", word), text);
            }

            if let Ok((_, name)) = parse_function(line) {
                functions.insert(name.to_string(), i+1);    
            }
            i += 1;
        }
        QuackInterp { lines, functions }
    }

    fn find_if_end(&self, i: &usize) -> usize {
        let mut depth = 1;
        let mut i = *i + 1;
        while i < self.lines.len() && depth != 0 {
            match parse_line(&self.lines[i]) {
                Ok((_, command)) => match command {
                    Command::If(_) => depth += 1,
                    Command::ElseIf(_) => if depth == 1 {return i},
                    Command::Else => if depth == 1 {return i},
                    Command::EndIf => depth -= 1,
                    _ => (),
                },
                Err(_) => (),
            };
            i += 1;
        }
        i
    }

    fn find_while_end(&self, i: &usize) -> usize {
        let mut depth = 1;
        let mut i = *i + 1;
        while i < self.lines.len() && depth != 0 {
            match parse_line(&self.lines[i]) {
                Ok((_, command)) => match command {
                    Command::While(_) => depth += 1,
                    Command::EndWhile => depth -= 1,
                    _ => (),
                },
                Err(_) => (),
            };
            i += 1;
        }
        i
    }
    
    fn find_function_end(&self, i: &usize) -> usize {
        let mut i = *i + 1;
        while i < self.lines.len(){
            match parse_line(&self.lines[i]) {
                Ok((_, command)) => match command {
                    Command::EndFunction => return i + 1,
                    _ => (),
                },
                Err(_) => (),
            };
            i += 1;
        }
        i
    }

    fn resolve_value(&self, value: Value, keyboard: &mut Keyboard, variables: &mut HashMap<String, i64>) -> Option<i64> {
        match value {
            Value::Int(int) =>  Some(int),
            Value::Variable(name) => variables.get(name).map(|int| *int),
            Value::Bracket(expression) => self.resolve_expr(*expression, keyboard, variables),
        }
    }
    
    fn resolve_expr(&self, expression: Expression, keyboard: &mut Keyboard, variables: &mut HashMap<String, i64>) -> Option<i64> {
        let mut amount = self.resolve_value(expression.value, keyboard, variables)?;
        

        for op in expression.ops {
            match op {
                Operator::Add(value) => amount += self.resolve_value(value, keyboard, variables)?,
                Operator::Sub(value) => amount -= self.resolve_value(value, keyboard, variables)?,
                Operator::Mult(value) => amount *= self.resolve_value(value, keyboard, variables)?,
                Operator::Div(value) => amount /= self.resolve_value(value, keyboard, variables)?,
                Operator::Mod(value) => amount %= self.resolve_value(value, keyboard, variables)?,
                Operator::Exp(value) => amount = amount.pow(self.resolve_value(value, keyboard, variables)? as u32),
                Operator::Equ(value) => amount = if amount == self.resolve_value(value, keyboard, variables)? {1} else {0},
                Operator::Not(value) => amount = if amount != self.resolve_value(value, keyboard, variables)? {1} else {0},
                Operator::Gre(value) => amount = if amount > self.resolve_value(value, keyboard, variables)? {1} else {0},
                Operator::Les(value) => amount = if amount < self.resolve_value(value, keyboard, variables)? {1} else {0},
                Operator::EqL(value) => amount = if amount <= self.resolve_value(value, keyboard, variables)? {1} else {0},
                Operator::EqG(value) => amount = if amount >= self.resolve_value(value, keyboard, variables)? {1} else {0},
                Operator::And(value) => amount = if amount != 0 && self.resolve_value(value, keyboard, variables)? != 0 {1} else {0},
                Operator::Or(value) => amount = if amount != 0 || self.resolve_value(value, keyboard, variables)? != 0 {1} else {0},
                Operator::BAnd(value) => amount &= self.resolve_value(value, keyboard, variables)?,
                Operator::BOr(value) => amount |= self.resolve_value(value, keyboard, variables)?,
                Operator::Left(value) => amount <<= self.resolve_value(value, keyboard, variables)?,
                Operator::Right(value) => amount >>= self.resolve_value(value, keyboard, variables)?,
            }
        }

        return Some(amount)
    }

    fn interpret(&self, i: &usize, line: &str, keyboard: &mut Keyboard, variables: &mut HashMap<String, i64>, stack: &mut Vec<usize>) -> usize {
        let command = match parse_line(line) {
            Ok((_, command)) => command,
            Err(_) => return i + 1,
        };

        match command {
            crate::parser::Command::Rem(comment) => println!("{}", comment),
            crate::parser::Command::String(str) => keyboard.press_string(str),
            crate::parser::Command::StringLN(str) => {
                keyboard.press_string(str); 
                keyboard.press_key(&Key::Special(SpecialKey::Enter));
            },
            crate::parser::Command::Special(special) => {keyboard.press_key(&Key::Special(special));},
            crate::parser::Command::Modifier(modifier) => keyboard.press_modifier(&modifier),
            crate::parser::Command::Shortcut(modifiers, key) => {keyboard.press_shortcut(&modifiers, &key);},
            crate::parser::Command::Delay(expression) => {
                let amount = self.resolve_expr(expression, keyboard, variables).unwrap_or(0);
                if amount < 20 {
                    return i + 1;
                }
                thread::sleep(Duration::from_millis(u64::try_from(amount).unwrap_or(20)));
            },
            crate::parser::Command::Hold(key) => {keyboard.hold(&key);},
            crate::parser::Command::Release(key) => keyboard.release(&key),
            crate::parser::Command::HoldMod(modifier) => keyboard.hold_mod(&modifier),
            crate::parser::Command::ReleaseMod(modifier) => keyboard.release_mod(&modifier),
            crate::parser::Command::InjectMod => (),
            crate::parser::Command::Var(name, expression) => {
                if !variables.contains_key(name) {
                    variables.insert(name.to_string(), 0);
                }

                let amount = self.resolve_expr(expression, keyboard, variables).unwrap_or(0);

                variables.insert(name.to_string(), amount);
            },
            crate::parser::Command::If(value) => {
                let cond = self.resolve_value(value, keyboard, variables).unwrap_or(0);
                if cond == 0 {
                    stack.push(0);
                    return self.find_if_end(i)
                }
                stack.push(1);
            },
            crate::parser::Command::ElseIf(value) => {
                let cond = self.resolve_value(value, keyboard, variables).unwrap_or(0);
                if cond == 0 || stack.pop().unwrap_or(0) == 1{
                    return self.find_if_end(i)
                }
                stack.push(1);
            },
            crate::parser::Command::Else => if stack.pop().unwrap_or(1) == 1 {
                return self.find_if_end(i)
            },
            crate::parser::Command::EndIf => {stack.pop();},
            crate::parser::Command::While(value) => {
                let cond = self.resolve_value(value, keyboard, variables).unwrap_or(0);
                if cond == 0 {
                    return self.find_while_end(i);
                }
                stack.push(*i);
            },
            crate::parser::Command::EndWhile => return stack.pop().unwrap_or(*i + 1),
            crate::parser::Command::Function(_) => return self.find_function_end(i),
            crate::parser::Command::EndFunction => return stack.pop().unwrap_or(*i + 1),
            crate::parser::Command::Call(name) => {
                stack.push(i+1);
                return *self.functions.get(name).unwrap_or(&(i + 1))
            },
        };
        i + 1
    }

    pub fn run(&self, hid: &mut HID) {
        let mut keyboard = Keyboard::new();
        let mut variables = HashMap::new();
        let mut stack = Vec::new();
        let mut i = 0;
        while i <  self.lines.len() {
            i = self.interpret(&i, &self.lines[i], &mut keyboard, &mut variables, &mut stack);
            keyboard.send(hid).unwrap();
        }
    }
}

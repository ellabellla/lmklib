use std::{collections::HashMap, time::Duration, thread, fmt::{Display}};

use lmk_hid::{key::{Keyboard, Key, SpecialKey}, HID};

use crate::parser::{parse_define, parse_line, Value, Expression, Operator, Command, parse_function};

pub enum Error{
    StackUnderflow,
    InvalidDelay(i64),
    CannotParse(usize),
    UnresolvedValue,
    UnknownFunction,
    NoEndIf,
    NoEndWhile,
    NoEndFunction,
}

impl Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::StackUnderflow => f.write_str("Stack underflow: This may be due to an error with mismatching control structure statements."),
            Error::InvalidDelay(delay) => f.write_str(&format!("Invalid Delay of {}: Delay length must be an integer greater than 20.", delay)),
            Error::CannotParse(line) => f.write_str(&format!("Parse Error: Could not parse line {}.", line)),
            Error::UnresolvedValue => f.write_str("Could not resolve value: This may be due to an invalid variable name."),
            Error::UnknownFunction => f.write_str("Unknown Function: The called function has not been defined."),
            Error::NoEndIf => f.write_str("No End If: Could not find matching END_IF for IF."),
            Error::NoEndWhile => f.write_str("No End While: Could not find matching END_WHILE for WHILE."),
            Error::NoEndFunction => f.write_str("No End Function: Could not find matching END_FUNCTION for FUNCTION."),
        }
    }
}

impl Error {
    pub fn to_err_msg(&self, line: &usize) -> String {
        format!("Error on line {}, {}", line, self)
    }
}

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

    fn find_if_end(&self, i: &usize) -> Option<usize> {
        let mut depth = 1;
        let mut i = *i + 1;
        while i < self.lines.len() && depth != 0 {
            match parse_line(&self.lines[i]) {
                Ok((_, command)) => match command {
                    Command::If(_) => depth += 1,
                    Command::ElseIf(_) => if depth == 1 {return Some(i)},
                    Command::Else => if depth == 1 {return Some(i)},
                    Command::EndIf => depth -= 1,
                    _ => (),
                },
                Err(_) => (),
            };
            i += 1;
        }
        if depth == 0 {
            Some(i)
        } else {
            None
        }
    }

    fn find_while_end(&self, i: &usize) -> Option<usize> {
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
        None
    }
    
    fn find_function_end(&self, i: &usize) -> Option<usize> {
        let mut i = *i + 1;
        while i < self.lines.len(){
            match parse_line(&self.lines[i]) {
                Ok((_, command)) => match command {
                    Command::EndFunction => return Some(i + 1),
                    _ => (),
                },
                Err(_) => (),
            };
            i += 1;
        }
        None
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

    fn interpret(&self, i: &usize, line: &str, keyboard: &mut Keyboard, variables: &mut HashMap<String, i64>, stack: &mut Vec<usize>) -> Result<usize, Error> {
        let command = match parse_line(line) {
            Ok((_, command)) => command,
            Err(_) => return Err(Error::CannotParse(*i)),
        };

        match command {
            Command::Rem(comment) => println!("{}", comment),
            Command::String(str) => keyboard.press_string(str),
            Command::StringLN(str) => {
                keyboard.press_string(str); 
                keyboard.press_key(&Key::Special(SpecialKey::Enter));
            },
            Command::Special(special) => {keyboard.press_key(&Key::Special(special));},
            Command::Modifier(modifier) => keyboard.press_modifier(&modifier),
            Command::Shortcut(modifiers, key) => {keyboard.press_shortcut(&modifiers, &key);},
            Command::Delay(expression) => {
                let amount = self.resolve_expr(expression, keyboard, variables).ok_or(Error::UnresolvedValue)?;
                if amount < 20 {
                    return Err(Error::InvalidDelay(amount))
                }
                
                let amount = u64::try_from(amount).map_err(|_| Error::InvalidDelay(amount))?;
                
                thread::sleep(Duration::from_millis(amount));
            },
            Command::Hold(key) => {keyboard.hold(&key);},
            Command::Release(key) => keyboard.release(&key),
            Command::HoldMod(modifier) => keyboard.hold_mod(&modifier),
            Command::ReleaseMod(modifier) => keyboard.release_mod(&modifier),
            Command::InjectMod => (),
            Command::Var(name, expression) => {
                if !variables.contains_key(name) {
                    variables.insert(name.to_string(), 0);
                }

                let amount = self.resolve_expr(expression, keyboard, variables).unwrap_or(0);

                variables.insert(name.to_string(), amount);
            },
            Command::If(value) => {
                let cond = self.resolve_value(value, keyboard, variables).ok_or(Error::UnresolvedValue)?;
                if cond == 0 {
                    stack.push(0);
                    return self.find_if_end(i).ok_or(Error::NoEndIf)
                }
                stack.push(1);
            },
            Command::ElseIf(value) => {
                let cond = self.resolve_value(value, keyboard, variables).ok_or(Error::UnresolvedValue)?;
                if cond == 0 || stack.pop().unwrap_or(0) == 1{
                    return self.find_if_end(i).ok_or(Error::NoEndIf)
                }
                stack.push(1);
            },
            Command::Else => if stack.pop().unwrap_or(1) == 1 {
                return self.find_if_end(i).ok_or(Error::NoEndIf)
            },
            Command::EndIf => {stack.pop();},
            Command::While(value) => {
                let cond = self.resolve_value(value, keyboard, variables).ok_or(Error::UnresolvedValue)?;
                if cond == 0 {
                    return self.find_while_end(i).ok_or(Error::NoEndWhile)
                }
                stack.push(*i);
            },
            Command::EndWhile => return stack.pop().ok_or(Error::StackUnderflow),
            Command::Function(_) => return self.find_function_end(i).ok_or(Error::NoEndFunction),
            Command::EndFunction => return stack.pop().ok_or(Error::StackUnderflow),
            Command::Call(name) => {
                stack.push(i+1);
                return self.functions.get(name).map(|i| *i).ok_or(Error::UnknownFunction)
            },
            Command::None => (),
        };
        Ok(i + 1)
    }

    pub fn run(&self, hid: &mut HID, continue_on_error: &bool) -> Result<(), (usize, Error)> {
        let mut keyboard = Keyboard::new();
        let mut variables = HashMap::new();
        let mut stack = Vec::new();
        let mut i = 0;
        while i <  self.lines.len() {
            match self.interpret(&i, &self.lines[i], &mut keyboard, &mut variables, &mut stack) {
                Ok(next) => i = next,
                Err(e) => {
                    if *continue_on_error {
                        println!("{}", e.to_err_msg(&i));
                        i += 1
                    } else {
                        return Err((i, e));
                    }
                },
            }
            keyboard.send(hid).unwrap();
        }
        Ok(())
    }
}

use std::{collections::HashMap, time::Duration, thread, fmt::{Display}};

use lmk_hid::{key::{Keyboard, Key, SpecialKey}, HID};

use crate::parser::{parse_define, parse_line, Value, Expression, Operator, Command, parse_function, string_variable};

pub enum Error{
    StackUnderflow,
    InvalidDelay(i64),
    CannotParse(usize),
    UnresolvedValue,
    UnknownFunction,
    NoEndIf,
    NoEndWhile,
    NoEndFunction,
    UnableToSend,
    UnexpectedReturn,
    ExpectedReturn,
    UnexpectedReturnIndex
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
            Error::UnableToSend => f.write_str("Unable to send key strokes: Make sure your keyboard is connected to a host."),
            Error::UnexpectedReturn => f.write_str("Unexpected return: This may be due to a misplaced RETURN or mismatched FUNCTION statements."),
            Error::ExpectedReturn => f.write_str("Expected return: Function did not return a value when it was expected. This may be due to mismatched function statements."),
            Error::UnexpectedReturnIndex => f.write_str("Unexpected Return Index: This may be due to an error with mismatching control structure statements."),
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

struct Runtime<'a> {
    keyboard: Keyboard,
    variables: HashMap<String, i64>,
    stack: Vec<usize>,
    i: usize,
    hid: &'a mut HID, 
    comments: bool, 
    errors: bool, 
    continue_on_error: bool,
    expect_return: bool,
}

impl<'a> Runtime<'a> {
    pub fn new(hid: &'a mut HID, comments: bool, errors: bool, continue_on_error: bool, expect_return: bool) -> Runtime<'a> {
        Runtime { keyboard: Keyboard::new(), variables: HashMap::new(), stack: vec![], i: 0, hid: hid, comments, errors, continue_on_error, expect_return }
    }
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

    fn resolve_value(&self, value: Value, rt: &mut Runtime) -> Result<i64, Error> {
        match value {
            Value::Int(int) =>  Ok(int),
            Value::Variable(name) => rt.variables.get(name).map(|int| *int).ok_or(Error::UnresolvedValue),
            Value::Bracket(expression) => self.resolve_expr(*expression, rt),
            Value::Call(name) => {
                let ret_idx = rt.i;
                let func_idx = self.functions.get(name).map(|i| *i).ok_or(Error::UnknownFunction)?;
                rt.i = func_idx;
                rt.stack.push(ret_idx);
                rt.expect_return = true;
                let ret = match self.run_intern(rt) {
                    Ok(ret) => ret.ok_or(Error::ExpectedReturn),
                    Err((_,e)) => return Err(e),
                }?;
                if rt.i != ret_idx {
                    return Err(Error::UnexpectedReturnIndex)
                }
                rt.expect_return = false;
                return Ok(ret)
            },
        }
    }
    
    fn resolve_expr(&self, expression: Expression, rt: &mut Runtime) -> Result<i64, Error> {
        let mut amount = self.resolve_value(expression.value, rt)?;
        

        for op in expression.ops {
            match op {
                Operator::Add(value) => amount += self.resolve_value(value, rt)?,
                Operator::Sub(value) => amount -= self.resolve_value(value, rt)?,
                Operator::Mult(value) => amount *= self.resolve_value(value, rt)?,
                Operator::Div(value) => amount /= self.resolve_value(value, rt)?,
                Operator::Mod(value) => amount %= self.resolve_value(value, rt)?,
                Operator::Exp(value) => amount = amount.pow(self.resolve_value(value, rt)? as u32),
                Operator::Equ(value) => amount = if amount == self.resolve_value(value, rt)? {1} else {0},
                Operator::Not(value) => amount = if amount != self.resolve_value(value, rt)? {1} else {0},
                Operator::Gre(value) => amount = if amount > self.resolve_value(value, rt)? {1} else {0},
                Operator::Les(value) => amount = if amount < self.resolve_value(value, rt)? {1} else {0},
                Operator::EqL(value) => amount = if amount <= self.resolve_value(value, rt)? {1} else {0},
                Operator::EqG(value) => amount = if amount >= self.resolve_value(value, rt)? {1} else {0},
                Operator::And(value) => amount = if amount != 0 && self.resolve_value(value, rt)? != 0 {1} else {0},
                Operator::Or(value) => amount = if amount != 0 || self.resolve_value(value, rt)? != 0 {1} else {0},
                Operator::BAnd(value) => amount &= self.resolve_value(value, rt)?,
                Operator::BOr(value) => amount |= self.resolve_value(value, rt)?,
                Operator::Left(value) => amount <<= self.resolve_value(value, rt)?,
                Operator::Right(value) => amount >>= self.resolve_value(value, rt)?,
            }
        }

        return Ok(amount)
    }

    fn press_string<'a>(&self, rt: &mut Runtime, str: &'a str) -> Result<(), Error> {
        if let Some(name) = string_variable(str) {
            let value = rt.variables.get(name).ok_or(Error::UnresolvedValue)?;
            rt.keyboard.press_string(&value.to_string());
        } else {
            rt.keyboard.press_string(str)
        }
        Ok(())
    }

    fn interpret(&self, line: &str, rt: &mut Runtime) -> Result<(usize, Option<i64>), Error> {
        let command = match parse_line(line) {
            Ok((_, command)) => command,
            Err(_) => return Err(Error::CannotParse(rt.i)),
        };

        match command {
            Command::Rem(comment) => if rt.comments {println!("{}", comment)},
            Command::String(str) => self.press_string(rt, str)?,
            Command::StringLN(str) => {
                self.press_string(rt, str)?;
                rt.keyboard.press_key(&Key::Special(SpecialKey::Enter));
            },
            Command::Special(special) => {rt.keyboard.press_key(&Key::Special(special));},
            Command::Modifier(modifier) => rt.keyboard.press_modifier(&modifier),
            Command::Shortcut(modifiers, key) => {rt.keyboard.press_shortcut(&modifiers, &key);},
            Command::Delay(expression) => {
                let amount = self.resolve_expr(expression, rt)?;
                if amount < 20 {
                    return Err(Error::InvalidDelay(amount))
                }
                
                let amount = u64::try_from(amount).map_err(|_| Error::InvalidDelay(amount))?;
                
                thread::sleep(Duration::from_millis(amount));
            },
            Command::Hold(key) => {rt.keyboard.hold(&key);},
            Command::Release(key) => rt.keyboard.release(&key),
            Command::HoldMod(modifier) => rt.keyboard.hold_mod(&modifier),
            Command::ReleaseMod(modifier) => rt.keyboard.release_mod(&modifier),
            Command::InjectMod => (),
            Command::Var(name, expression) => {
                if !rt.variables.contains_key(name) {
                    rt.variables.insert(name.to_string(), 0);
                }

                let amount = self.resolve_expr(expression, rt)?;

                rt.variables.insert(name.to_string(), amount);
            },
            Command::If(value) => {
                let cond = self.resolve_value(value, rt)?;
                if cond == 0 {
                    rt.stack.push(0);
                    return self.find_if_end(&rt.i).ok_or(Error::NoEndIf).map(|i| (i, None))
                }
                rt.stack.push(1);
            },
            Command::ElseIf(value) => {
                let cond = self.resolve_value(value, rt)?;
                if cond == 0 || rt.stack.pop().ok_or(Error::StackUnderflow)? == 1{
                    return self.find_if_end(&rt.i).ok_or(Error::NoEndIf).map(|i| (i, None))
                }
                rt.stack.push(1);
            },
            Command::Else => if rt.stack.pop().ok_or(Error::StackUnderflow)? == 1 {
                return self.find_if_end(&rt.i).ok_or(Error::NoEndIf).map(|i| (i, None))
            },
            Command::EndIf => {rt.stack.pop();},
            Command::While(value) => {
                let cond = self.resolve_value(value, rt)?;
                if cond == 0 {
                    return self.find_while_end(&rt.i).ok_or(Error::NoEndWhile).map(|i| (i, None))
                }
                rt.stack.push(rt.i);
            },
            Command::EndWhile => return rt.stack.pop().ok_or(Error::StackUnderflow).map(|i| (i, None)),
            Command::Function(_) => return self.find_function_end(&rt.i).ok_or(Error::NoEndFunction).map(|i| (i, None)),
            Command::EndFunction => {
                if rt.expect_return {
                    return Err(Error::ExpectedReturn)
                }
                return rt.stack.pop().ok_or(Error::StackUnderflow).map(|i| (i, None))
            },
            Command::Call(name) => {
                rt.stack.push(&rt.i+1);
                return self.functions.get(name).map(|i| *i).ok_or(Error::UnknownFunction).map(|i| (i, None))
            },
            Command::None => (),
            Command::Return(value) => {
                let value = self.resolve_value(value, rt)?;
                return rt.stack.pop().ok_or(Error::StackUnderflow).map(|i| (i, Some(value)))
            },
        };
        Ok((&rt.i + 1, None))
    }

    pub fn run(&self, hid: &mut HID, comments: &bool, errors: &bool, continue_on_error: &bool) -> Result<(), (usize, Error)> {
        let mut rt = Runtime::new(hid, *comments, *errors, *continue_on_error, false);
        self.run_intern(&mut rt).map(|_| ())
    }

    fn run_intern(&self, rt: &mut Runtime) -> Result<Option<i64>, (usize, Error)>  {
        while rt.i <  self.lines.len() {
            match self.interpret(&self.lines[rt.i], rt) {
                Ok((next, ret)) => {
                    let old_i = rt.i;
                    rt.i = next;

                    if let Some(ret) = ret {
                        if rt.expect_return {
                            return Ok(Some(ret))
                        } else if rt.continue_on_error{
                            println!("{}", Error::UnexpectedReturn.to_err_msg(&old_i));
                        } else {
                            return Err((old_i, Error::UnexpectedReturn))
                        }
                    }
                },
                Err(e) => {
                    if rt.continue_on_error {
                        if rt.errors {
                            println!("{}", e.to_err_msg(&rt.i));
                        }
                        rt.i += 1
                    } else {
                        return Err((rt.i, e));
                    }
                },
            }
            match rt.keyboard.send(rt.hid).map_err(|_|Error::UnableToSend) {
                Ok(_) => (),
                Err(e) => if rt.continue_on_error {
                    if rt.errors {
                        println!("{}", e.to_err_msg(&rt.i));
                    }
                    rt.i += 1
                } else {
                    return Err((rt.i, e));
                },
            };
        }
        Ok(None)
    }
}

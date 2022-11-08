use std::{collections::HashMap};

use lmk_hid::{key::{Keyboard, KeyOrigin}, mouse::{Mouse, MouseButton}, HID};

use crate::parser::{BorkParser, Command, Key, Expression, NestType, Parameter, ParameterName, FuncBody, Value, DataType, Operator, };

#[derive(Debug)]
pub enum Error {
    ParseError,
    UndefinedVariable,
    ExpectedInteger,
    MismatchedWhile,
    IOError(std::io::Error),
    UndefinedFunction,
    InvalidValue,
}

enum Data<'a> {
    Integer(i64),
    Literal(Vec<Key<'a>>),
}

pub struct BorkInterp<'a> {
    source: &'a str
}

impl<'a> BorkInterp<'a> {
    pub fn new(source: &'a str) -> BorkInterp<'a> {
        BorkInterp { source }
    }

    pub fn run(&self) -> Result<(), Error> {
        let mut keyboard =  Keyboard::new();
        let mut mouse = Mouse::new();
        let mut parser = BorkParser::new();
        let mut variables = HashMap::new();
        let mut if_stack = Vec::new();
        let mut while_stack = Vec::new();
        let mut hid = HID::new(1,0).map_err(|e| Error::IOError(e))?;

        let mut i = self.source;
        let mut cont;
        loop {
            (cont, i) = BorkInterp::interp_command(&mut variables, &mut keyboard, &mut mouse, &mut parser, &mut if_stack, &mut while_stack, i)?;
            keyboard.send(&mut hid).map_err(|e| Error::IOError(e))?;
            mouse.send(&mut hid).map_err(|e| Error::IOError(e))?;
            if !cont {
                break;
            }
        }

        Ok(())
    }

    fn press_key(
        variables: &mut HashMap<String, Data<'a>>, 
        keyboard: &mut Keyboard, 
        mouse: &mut Mouse, 
        parser: &mut BorkParser<'a>,
        key: &Key<'a>
    ) -> Result<(), Error> {
        match key {
            Key::Modifier(modi) => keyboard.press_modifier(&modi),
            Key::Special(s) => {keyboard.press_key(&lmk_hid::key::Key::Special(*s));},
            Key::Literal(c) => {keyboard.press_key(&lmk_hid::key::Key::Char(*c, KeyOrigin::Keyboard));},
            Key::ASCII(exp) => {
                let c = BorkInterp::resolve_ascii(variables, keyboard, mouse, parser, &exp)?;
                keyboard.press_key(&lmk_hid::key::Key::Char(c, KeyOrigin::Keyboard));
            },
            Key::Variable(name) => match BorkInterp::resolve_variable(variables, name)? {
                Data::Integer(i) => keyboard.press_string(&format!("{}", i)),
                Data::Literal(keys) => for key in keys { BorkInterp::press_key(variables, keyboard, mouse, parser, &key)? },
            },
            Key::Keycode(byte) => keyboard.press_keycode(*byte),
            Key::Left => mouse.press_button(&MouseButton::Left),
            Key::Right => mouse.press_button(&MouseButton::Right),
            Key::Middle => mouse.press_button(&MouseButton::Middle),
        };

        Ok(())
    }

    fn hold_key(
        variables: &mut HashMap<String, Data<'a>>, 
        keyboard: &mut Keyboard, 
        mouse: &mut Mouse, 
        parser: &mut BorkParser<'a>,
        key: &Key<'a>
    ) -> Result<(), Error> {
        match key {
            Key::Modifier(modi) => keyboard.hold_mod(&modi),
            Key::Special(s) => {keyboard.hold(&lmk_hid::key::Key::Special(*s));},
            Key::Literal(c) => {keyboard.hold(&lmk_hid::key::Key::Char(*c, KeyOrigin::Keyboard));},
            Key::ASCII(exp) => {
                let c = BorkInterp::resolve_ascii(variables, keyboard, mouse, parser, &exp)?;
                keyboard.hold(&lmk_hid::key::Key::Char(c, KeyOrigin::Keyboard));
            },
            Key::Variable(name) => match BorkInterp::resolve_variable(variables, name)? {
                Data::Integer(i) => keyboard.hold_string(&format!("{}", i)),
                Data::Literal(keys) => for key in keys { BorkInterp::hold_key(variables, keyboard, mouse, parser, &key)? },
            },
            Key::Keycode(byte) => keyboard.hold_keycode(*byte),
            Key::Left => mouse.hold_button(&MouseButton::Left),
            Key::Right => mouse.hold_button(&MouseButton::Right),
            Key::Middle => mouse.hold_button(&MouseButton::Middle),
        };

        Ok(())
    }

    fn release_key(
        variables: &mut HashMap<String, Data<'a>>, 
        keyboard: &mut Keyboard, 
        mouse: &mut Mouse, 
        parser: &mut BorkParser<'a>,
        key: &Key<'a>
    ) -> Result<(), Error> {
        match key {
            Key::Modifier(modi) => keyboard.release_mod(&modi),
            Key::Special(s) => {keyboard.release(&lmk_hid::key::Key::Special(*s));},
            Key::Literal(c) => {keyboard.release(&lmk_hid::key::Key::Char(*c, KeyOrigin::Keyboard));},
            Key::ASCII(exp) => {
                let c = BorkInterp::resolve_ascii(variables, keyboard, mouse, parser, &exp)?;
                keyboard.release(&lmk_hid::key::Key::Char(c, KeyOrigin::Keyboard));
            },
            Key::Variable(name) => match BorkInterp::resolve_variable(variables, name)? {
                Data::Integer(i) => keyboard.release_string(&format!("{}", i)),
                Data::Literal(keys) => for key in keys { BorkInterp::release_key(variables, keyboard, mouse, parser, &key)? },
            },
            Key::Keycode(byte) => keyboard.release_keycode(*byte),
            Key::Left => mouse.release_button(&MouseButton::Left),
            Key::Right => mouse.release_button(&MouseButton::Right),
            Key::Middle => mouse.release_button(&MouseButton::Middle),
        };

        Ok(())
    }

    fn resolve_ascii(
        variables: &mut HashMap<String, Data<'a>>, 
        keyboard: &mut Keyboard, 
        mouse: &mut Mouse, 
        parser: &mut BorkParser<'a>,
        exp: &Expression<'a>
    ) -> Result<char, Error> {
        let num = BorkInterp::resolve_expression(variables, keyboard, mouse, parser, exp)?;
        
        let num = if num < u8::MIN as i64 {
            u8::MIN
        } else if num >= u8::MAX as i64 {
            u8::MAX
        } else {
            num as u8
        };


        Ok(num as char)
    }

    fn resolve_variable_int(variables: &HashMap<String, Data<'a>>,  name: &'a str) -> Result<i64, Error> {
        variables.get(name)
            .ok_or(Error::UndefinedVariable)
            .and_then(|d| match d {
                Data::Integer(i) => Ok(*i),
                Data::Literal(_) => Err(Error::ExpectedInteger),
            })
    }

    fn resolve_variable(variables: &HashMap<String, Data<'a>>, name: &'a str) -> Result<Data<'a>, Error> {
        variables.get(name).ok_or(Error::UndefinedVariable).map(|d| match d {
            Data::Integer(i) => Data::Integer(*i),
            Data::Literal(l) => Data::Literal(l.clone()),
        })
    }

    fn resolve_value(
        variables: &mut HashMap<String, Data<'a>>, 
        keyboard: &mut Keyboard, 
        mouse: &mut Mouse, 
        parser: &mut BorkParser<'a>,  
        val: &Value<'a>
    ) -> Result<i64, Error> {
        Ok(match val {
            Value::Int(i) => *i,
            Value::Variable(name) => BorkInterp::resolve_variable_int(variables, name)?,
            Value::Call(name, params) => {
                // copy params
                let mut p = Vec::with_capacity(params.len());
                p.extend(params);
                let params = p.into_iter().map(|p| p.clone()).collect();

                if let Some(value) = BorkInterp::resolve_call(variables, keyboard, mouse, parser, &mut Vec::new(), &mut Vec::new(), DataType::Integer, name, params)? {
                    value
                } else {
                    return Err(Error::InvalidValue)
                }
            },
            Value::Bracket(exp) => BorkInterp::resolve_expression(variables, keyboard, mouse, parser, exp)?,
            Value::LED(_) => todo!(),
        })
    }

    fn resolve_bool(bool: bool) -> i64 {
        if bool {
            1
        } else {
            0
        }
    }

    fn resolve_to_bool(value: i64) -> i64 {
        if value > 0 {
            1
        } else {
            0
        }
    }

    fn resolve_operator(
        variables: &mut HashMap<String, Data<'a>>, 
        keyboard: &mut Keyboard, 
        mouse: &mut Mouse, 
        parser: &mut BorkParser<'a>,
        mut value: i64,
        operator: &Operator<'a>
    ) -> Result<i64, Error> {
        match operator {
            Operator::Add(val) => value += BorkInterp::resolve_value(variables, keyboard, mouse, parser, val)?,
            Operator::Sub(val) => value -= BorkInterp::resolve_value(variables, keyboard, mouse, parser, val)?,
            Operator::Mult(val) => value *= BorkInterp::resolve_value(variables, keyboard, mouse, parser, val)?,
            Operator::Div(val) => value /= BorkInterp::resolve_value(variables, keyboard, mouse, parser, val)?,
            Operator::Mod(val) => value %= BorkInterp::resolve_value(variables, keyboard, mouse, parser, val)?,
            Operator::Exp(val) => value = value.pow(BorkInterp::resolve_value(variables, keyboard, mouse, parser, val)? as u32),
            Operator::Equ(val) => value = BorkInterp::resolve_bool(value == BorkInterp::resolve_value(variables, keyboard, mouse, parser, val)?),
            Operator::NEq(val) => value = BorkInterp::resolve_bool(value != BorkInterp::resolve_value(variables, keyboard, mouse, parser, val)?),
            Operator::Gre(val) => value = BorkInterp::resolve_bool(value > BorkInterp::resolve_value(variables, keyboard, mouse, parser, val)?),
            Operator::Les(val) => value = BorkInterp::resolve_bool(value < BorkInterp::resolve_value(variables, keyboard, mouse, parser, val)?),
            Operator::EqL(val) => value = BorkInterp::resolve_bool(value <= BorkInterp::resolve_value(variables, keyboard, mouse, parser, val)?),
            Operator::EqG(val) => value = BorkInterp::resolve_bool(value >= BorkInterp::resolve_value(variables, keyboard, mouse, parser, val)?),
            Operator::And(val) => value = BorkInterp::resolve_bool(
                BorkInterp::resolve_to_bool(value) == BorkInterp::resolve_to_bool(BorkInterp::resolve_value(variables, keyboard, mouse, parser, val)?)
            ),
            Operator::Or(val) => value = if BorkInterp::resolve_to_bool(value) == 1 {
                1
            }  else {
                BorkInterp::resolve_to_bool(BorkInterp::resolve_value(variables, keyboard, mouse, parser, val)?)
            },
            Operator::BAnd(val) => value &= BorkInterp::resolve_value(variables, keyboard, mouse, parser, val)?,
            Operator::BOr(val) => value |= BorkInterp::resolve_value(variables, keyboard, mouse, parser, val)?,
            Operator::Left(val) => value <<= BorkInterp::resolve_value(variables, keyboard, mouse, parser, val)?,
            Operator::Right(val) => value >>= BorkInterp::resolve_value(variables, keyboard, mouse, parser, val)?,
            Operator::Set(name) => {variables.insert(name.to_string(), Data::Integer(value));},
            Operator::While(cond,op) => {
                while BorkInterp::resolve_to_bool(BorkInterp::resolve_expression(variables, keyboard, mouse, parser, cond)?) == 1 {
                    value = BorkInterp::resolve_operator(variables, keyboard, mouse, parser, value, op)?;
                }
            },
            Operator::If(t,f) => {
                if BorkInterp::resolve_to_bool(value) == 1 {
                    value = BorkInterp::resolve_expression(variables, keyboard, mouse, parser, t)?;
                } else {
                    value = BorkInterp::resolve_expression(variables, keyboard, mouse, parser, f)?;
                }
            },
        }

        Ok(value)
    }

    fn resolve_expression(
        variables: &mut HashMap<String, Data<'a>>, 
        keyboard: &mut Keyboard, 
        mouse: &mut Mouse, 
        parser: &mut BorkParser<'a>,
        exp: &Expression<'a>
    ) -> Result<i64, Error> {
        let mut value = BorkInterp::resolve_value(variables, keyboard, mouse, parser, &exp.value)?;

        for op in exp.ops.iter() {
            value = BorkInterp::resolve_operator(variables, keyboard, mouse, parser, value, op)?;
        }
        Ok(0)
    }

    fn resolve_call(
        variables: &mut HashMap<String, Data<'a>>, 
        keyboard: &mut Keyboard, 
        mouse: &mut Mouse, 
        parser: &mut BorkParser<'a>, 
        if_stack: &mut Vec<usize>,
        while_stack: &mut Vec<&'a str>,
        expected: DataType,
        name: &'a str,
        params: Vec<Parameter<'a>>
    ) -> Result<Option<i64>, Error>{
        if let Some((fn_type, param_names, body)) = parser.remove_func(name) {
            match expected {
                DataType::Integer => match fn_type {
                    DataType::Integer => (),
                    DataType::Literal => return Ok(None),
                    DataType::Any => return Ok(None),
                },
                DataType::Literal => match fn_type {
                    DataType::Integer => return Ok(None),
                    DataType::Literal => (),
                    DataType::Any => return Ok(None),
                },
                DataType::Any => (),
            }

            let mut names = Vec::with_capacity(param_names.len());

            let names = {
                for (param, name) in params.into_iter().zip(param_names.iter()) {
                    let name = match name {
                        ParameterName::Expression(name) => name,
                        ParameterName::Literal(name) => name,
                    };
                    let data = match param {
                        Parameter::Expression(exp) => Data::Integer(match BorkInterp::resolve_expression(variables, keyboard, mouse, parser, &exp) {
                            Ok(res) => res,
                            Err(e) => {
                                parser.add_func(name, fn_type, param_names, body);
                                return Err(e)
                            },
                        }),
                        Parameter::Literal(keys) => Data::Literal(keys),
                    };
                    names.push(name);
                    variables.insert(name.to_string(), data);
                }

                names
            };

            
            let res = match &body {
                FuncBody::Expression(exp) => Some(match BorkInterp::resolve_expression(variables, keyboard, mouse, parser, &exp){
                    Ok(res) => res,
                    Err(e) => {
                        parser.add_func(name, fn_type, param_names, body);
                        return Err(e)
                    },
                }),
                FuncBody::Literal(coms) => {
                    let mut res = match BorkInterp::interp_command(variables, keyboard, mouse, parser, if_stack, while_stack, &coms){
                        Ok(res) => res,
                        Err(e) => {
                            parser.add_func(name, fn_type, param_names, body);
                            return Err(e)
                        },
                    };
                    while let (true, new_coms) =  res {
                        res = match BorkInterp::interp_command(variables, keyboard, mouse, parser, if_stack, while_stack, &new_coms) {
                            Ok(res) => res,
                            Err(e) => {
                                parser.add_func(name, fn_type, param_names, body);
                                return Err(e)
                            },
                        };
                    }
                    None
                },
            };


            for name in names {
                variables.remove(*name);
            }

            parser.add_func(name, fn_type, param_names, body);
            Ok(res)
        } else {
            Err(Error::UndefinedFunction)
        }
    }

    fn interp_command(
        variables: &mut HashMap<String, Data<'a>>, 
        keyboard: &mut Keyboard, 
        mouse: &mut Mouse, 
        parser: &mut BorkParser<'a>, 
        if_stack: &mut Vec<usize>,
        while_stack: &mut Vec<&'a str>,
        i: &'a str,
    ) -> Result<(bool, &'a str), Error>{
        let inside_while = matches!(parser.get_level_type(), Some(NestType::While));
        let (new_i, com) = parser.parse_command(i).map_err(|_| Error::ParseError)?;
        match com {
            Command::String(chars) => keyboard.press_string(&chars.iter().collect::<String>()),
            Command::Literal(keys) => for key in keys { BorkInterp::press_key(variables, keyboard, mouse, parser, &key)? },
            Command::Key(keys) => for key in keys { BorkInterp::press_key(variables, keyboard, mouse, parser, &key)? },
            Command::Hold(keys) => for key in keys { BorkInterp::hold_key(variables, keyboard, mouse, parser, &key)? },
            Command::Release(keys) => for key in keys { BorkInterp::release_key(variables, keyboard, mouse, parser, &key)? },
            Command::If(exp) => {
                let cond = BorkInterp::resolve_expression(variables, keyboard, mouse, parser, &exp)?;
                if cond == 0 {
                    let (new_i, _) = parser.jmp_next(new_i).map_err(|_| Error::ParseError)?;
                    return Ok((true, new_i))
                } else {
                    if_stack.push(parser.get_level())
                }
            },
            Command::ElseIf(exp) => {
                if let Some(level) = if_stack.last() {
                    if parser.get_level() == *level {
                        if_stack.pop();
                        let (new_i, _) = parser.jmp_end(new_i).map_err(|_| Error::ParseError)?;
                        return Ok((true, new_i))
                    }
                }

                let cond = BorkInterp::resolve_expression(variables, keyboard, mouse, parser, &exp)?;
                if cond == 0 {
                    let (new_i, _) = parser.jmp_next(new_i).map_err(|_| Error::ParseError)?;
                    return Ok((true, new_i))
                } else {
                    if_stack.push(parser.get_level())
                }
            },
            Command::Else => {
                if let Some(level) = if_stack.last() {
                    if parser.get_level() == *level {
                        if_stack.pop();
                        let (new_i, _) = parser.jmp_end(new_i).map_err(|_| Error::ParseError)?;
                        return Ok((true, new_i))
                    }
                }   
            },
            Command::While(exp) => {
                let cond = BorkInterp::resolve_expression(variables, keyboard, mouse, parser, &exp)?;
                if cond == 0 {
                    let (new_i, _) = parser.jmp_end(new_i).map_err(|_| Error::ParseError)?;
                    return Ok((true, new_i))
                } else {
                    while_stack.push(i);
                }
            }
            Command::End => {
                if inside_while {
                    return Ok((true, while_stack.pop().ok_or(Error::MismatchedWhile)?))
                } else {
                    if let Some(level) = if_stack.last() {
                        if parser.get_level() == *level {
                            if_stack.pop();
                        }
                    }
                }
            },
            Command::Set(name, exp) => {
                let value = BorkInterp::resolve_expression(variables, keyboard, mouse, parser, &exp)?;
                variables.insert(name.to_string(), Data::Integer(value));
            },
            Command::Print(name) => BorkInterp::press_key(variables, keyboard, mouse, parser, &Key::Variable(name))?,
            Command::Expression(exp) => {
                let value = BorkInterp::resolve_expression(variables, keyboard, mouse, parser, &exp)?;
                keyboard.press_string(&format!("{}", value));
            },
            Command::Left => mouse.press_button(&MouseButton::Left),
            Command::Right => mouse.press_button(&MouseButton::Right),
            Command::Middle => mouse.press_button(&MouseButton::Middle),
            Command::Move(x, y) => {
                let x = BorkInterp::resolve_expression(variables, keyboard, mouse, parser, &x)?;
                let x = if x < i8::MIN as i64{
                    i8::MIN
                } else if x > i8::MAX as i64{
                    i8::MAX
                } else {
                    x as i8
                };

                let y = BorkInterp::resolve_expression(variables, keyboard, mouse, parser, &y)?;
                let y = if y < i8::MIN as i64{
                    i8::MIN
                } else if y > i8::MAX as i64{
                    i8::MAX
                } else {
                    y as i8
                };

                mouse.move_mouse(&x, &lmk_hid::mouse::MouseDir::X);
                mouse.move_mouse(&y, &lmk_hid::mouse::MouseDir::Y);
            },
            Command::Pipe => todo!(),
            Command::Call(name, params) => {
                if let Some(value) = BorkInterp::resolve_call(variables, keyboard, mouse, parser, &mut Vec::new(), &mut Vec::new(), DataType::Any, name, params)? {
                    keyboard.press_string(&format!("{}", value));
                }
            },
            Command::None => (),
            Command::LED(_) => todo!(),
            Command::ASCII(exp) => {
                let c = BorkInterp::resolve_ascii(variables, keyboard, mouse, parser, &exp)?; 
                keyboard.press_key(&lmk_hid::key::Key::Char(c, KeyOrigin::Keyboard));
            },
            Command::Exit => return Ok((false, new_i)),
        }
        Ok((true, new_i))
    }
}

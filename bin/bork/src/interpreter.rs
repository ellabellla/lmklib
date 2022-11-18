use std::{collections::HashMap, process, time::Duration, thread};

use virt_hid::{key::{Keyboard, KeyOrigin}, mouse::{Mouse, MouseButton}, HID};

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
    PipeError(std::io::Error),
    PipeParseError(std::string::FromUtf8Error),
}

#[derive(Debug, Clone)]
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

    pub fn run(&self, hid: &mut HID) -> Result<(), Error> {
        let mut keyboard =  Keyboard::new();
        let mut mouse = Mouse::new();
        let mut parser = BorkParser::new();
        let mut variables = HashMap::new();
        let mut if_stack = Vec::new();
        let mut while_stack = Vec::new();

        let mut i = self.source;
        let mut cont;
        loop {
            keyboard.update_led_state(hid, Duration::from_millis(10)).map_err(|e| Error::IOError(e))?;
            (cont, i) = BorkInterp::interp_command(
                &mut variables, 
                &mut keyboard, 
                &mut mouse, 
                &mut parser, 
                &mut if_stack, 
                &mut while_stack, 
                i)?;
            keyboard.send(hid).map_err(|e| Error::IOError(e))?;
            mouse.send(hid).map_err(|e| Error::IOError(e))?;
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
            Key::Special(s) => {keyboard.press_key(&virt_hid::key::BasicKey::Special(*s));},
            Key::Literal(c) => {keyboard.press_key(&virt_hid::key::BasicKey::Char(*c, KeyOrigin::Keyboard));},
            Key::ASCII(exp) => {
                let c = BorkInterp::resolve_ascii(variables, keyboard, mouse, parser, &exp)?;
                if let Some(c) = c {
                    keyboard.press_key(&virt_hid::key::BasicKey::Char(c, KeyOrigin::Keyboard));
                }
            },
            Key::Variable(name) => match BorkInterp::resolve_variable(variables, name)? {
                Data::Integer(i) => keyboard.press_basic_string(&format!("{}", i)),
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
            Key::Special(s) => {keyboard.hold_key(&virt_hid::key::BasicKey::Special(*s));},
            Key::Literal(c) => {keyboard.hold_key(&virt_hid::key::BasicKey::Char(*c, KeyOrigin::Keyboard));},
            Key::ASCII(exp) => {
                let c = BorkInterp::resolve_ascii(variables, keyboard, mouse, parser, &exp)?;
                if let Some(c) = c {
                    keyboard.hold_key(&virt_hid::key::BasicKey::Char(c, KeyOrigin::Keyboard));
                }
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
            Key::Special(s) => {keyboard.release_key(&virt_hid::key::BasicKey::Special(*s));},
            Key::Literal(c) => {keyboard.release_key(&virt_hid::key::BasicKey::Char(*c, KeyOrigin::Keyboard));},
            Key::ASCII(exp) => {
                let c = BorkInterp::resolve_ascii(variables, keyboard, mouse, parser, &exp)?;
                if let Some(c) = c {
                   keyboard.release_key(&virt_hid::key::BasicKey::Char(c, KeyOrigin::Keyboard));
                }
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
    ) -> Result<Option<char>, Error> {
        BorkInterp::resolve_expression(variables, keyboard, mouse, parser, exp).map(|num| { 
            num.map(|num| {
                let num = if num < u8::MIN as i64 {
                    u8::MIN
                } else if num >= u8::MAX as i64 {
                    u8::MAX
                } else {
                    num as u8
                };
                num as char
            })
        })
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
    ) -> Result<Option<i64>, Error> {
        Ok(match val {
            Value::Int(i) => Some(*i),
            Value::Variable(name) => Some(BorkInterp::resolve_variable_int(variables, name)?),
            Value::Call(name, params) => {
                // copy params
                let mut p = Vec::with_capacity(params.len());
                p.extend(params);
                let params = p.into_iter().map(|p| p.clone()).collect();

                if let Some(value) = BorkInterp::resolve_call(variables, keyboard, mouse, parser, &mut Vec::new(), &mut Vec::new(), DataType::Integer, name, params)? {
                    Some(value)
                } else {
                    return Err(Error::InvalidValue)
                }
            },
            Value::Bracket(exp) => BorkInterp::resolve_expression(variables, keyboard, mouse, parser, exp)?,
            Value::LED(state) => {
                Some(BorkInterp::resolve_bool(keyboard.led_state(&state)))
            },
            Value::BNot(exp) => BorkInterp::resolve_expression(variables, keyboard, mouse, parser, exp)?.map(|v| !v),
            Value::Not(exp) => if BorkInterp::resolve_to_bool(BorkInterp::resolve_expression(variables, keyboard, mouse, parser, exp)?) == 1 {
                Some(0)
            } else {
                Some(1)
            },
        })
    }

    fn resolve_bool(bool: bool) -> i64 {
        if bool {
            1
        } else {
            0
        }
    }

    fn resolve_to_bool(value: Option<i64>) -> i64 {
        if let Some(value) = value {
            if value > 0 {
                1
            } else {
                0
            }
        } else {
            0
        }
    }

    fn resolve_operator(
        variables: &mut HashMap<String, Data<'a>>, 
        keyboard: &mut Keyboard, 
        mouse: &mut Mouse, 
        parser: &mut BorkParser<'a>,
        value: Option<i64>,
        operator: &Operator<'a>
    ) -> Result<Option<i64>, Error> {
        if let Some(value) = value {
            let value = match operator {
                Operator::Add(val) => BorkInterp::resolve_value(variables, keyboard, mouse, parser, val)?.map(|val| value + val),
                Operator::Sub(val) => BorkInterp::resolve_value(variables, keyboard, mouse, parser, val)?.map(|val| value - val),
                Operator::Mult(val) => BorkInterp::resolve_value(variables, keyboard, mouse, parser, val)?.map(|val| value * val),
                Operator::Div(val) => BorkInterp::resolve_value(variables, keyboard, mouse, parser, val)?.map(|val| value / val),
                Operator::Mod(val) => BorkInterp::resolve_value(variables, keyboard, mouse, parser, val)?.map(|val| value % val),
                Operator::Exp(val) => BorkInterp::resolve_value(variables, keyboard, mouse, parser, val)?.map(|val| value.saturating_pow(val as u32)),
                Operator::Equ(val) => BorkInterp::resolve_value(variables, keyboard, mouse, parser, val)?.map(|val|  BorkInterp::resolve_bool(value == val)),
                Operator::NEq(val) => BorkInterp::resolve_value(variables, keyboard, mouse, parser, val)?.map(|val|  BorkInterp::resolve_bool(value != val)),
                Operator::Gre(val) => BorkInterp::resolve_value(variables, keyboard, mouse, parser, val)?.map(|val|  BorkInterp::resolve_bool(value > val)),
                Operator::Les(val) => BorkInterp::resolve_value(variables, keyboard, mouse, parser, val)?.map(|val|  BorkInterp::resolve_bool(value < val)),
                Operator::EqL(val) => BorkInterp::resolve_value(variables, keyboard, mouse, parser, val)?.map(|val|  BorkInterp::resolve_bool(value <= val)),
                Operator::EqG(val) => BorkInterp::resolve_value(variables, keyboard, mouse, parser, val)?.map(|val|  BorkInterp::resolve_bool(value >= val)),
                Operator::And(val) => Some(BorkInterp::resolve_bool(
                    BorkInterp::resolve_to_bool(Some(value)) == 1 && BorkInterp::resolve_to_bool(BorkInterp::resolve_value(variables, keyboard, mouse, parser, val)?) == 1
                )),
                Operator::Or(val) => Some(BorkInterp::resolve_bool(
                    BorkInterp::resolve_to_bool(Some(value)) == 1 || BorkInterp::resolve_to_bool(BorkInterp::resolve_value(variables, keyboard, mouse, parser, val)?) == 1
                )),
                Operator::BAnd(val) => BorkInterp::resolve_value(variables, keyboard, mouse, parser, val)?.map(|val| value & val),
                Operator::BOr(val) => BorkInterp::resolve_value(variables, keyboard, mouse, parser, val)?.map(|val| value | val),
                Operator::Left(val) => BorkInterp::resolve_value(variables, keyboard, mouse, parser, val)?.map(|val| value << val),
                Operator::Right(val) => BorkInterp::resolve_value(variables, keyboard, mouse, parser, val)?.map(|val| value >> val),
                Operator::Set(name) => {variables.insert(name.to_string(), Data::Integer(value)); None},
                Operator::While(cond,op) => {
                    let mut value = Some(value);
                    while BorkInterp::resolve_to_bool(BorkInterp::resolve_expression(variables, keyboard, mouse, parser, cond)?) == 1 {
                        value = BorkInterp::resolve_operator(variables, keyboard, mouse, parser, value, op)?;
                    }

                    return Ok(value)
                },
                Operator::If(t,f) => {
                    let mut value = Some(value);
                    if BorkInterp::resolve_to_bool(value) == 1 {
                        value = BorkInterp::resolve_expression(variables, keyboard, mouse, parser, t)?;
                    } else {
                        value = BorkInterp::resolve_expression(variables, keyboard, mouse, parser, f)?;
                    }
                    return Ok(value);
                },
            };
            Ok(value)
        } else {
            Ok(None)
        }
    }

    fn resolve_expression(
        variables: &mut HashMap<String, Data<'a>>, 
        keyboard: &mut Keyboard, 
        mouse: &mut Mouse, 
        parser: &mut BorkParser<'a>,
        exp: &Expression<'a>
    ) -> Result<Option<i64>, Error> {
        let mut value = BorkInterp::resolve_value(variables, keyboard, mouse, parser, &exp.value)?;

        for op in exp.ops.iter() {
            value = BorkInterp::resolve_operator(variables, keyboard, mouse, parser, value, op)?;
        }
        Ok(value)
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
        let mut func_variables = HashMap::new();
        func_variables.extend(variables.iter().map(|(n, d)| (n.to_string(), d.clone())));
        let variables = &mut func_variables;
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

            for (param, name) in params.into_iter().zip(param_names.iter()) {
                let name = match name {
                    ParameterName::Expression(name) => name,
                    ParameterName::Literal(name) => name,
                };
                let data = match param {
                    Parameter::Expression(exp) => Data::Integer(match BorkInterp::resolve_expression(variables, keyboard, mouse, parser, &exp) {
                        Ok(res) => res.unwrap_or(0),
                        Err(e) => {
                            parser.add_func(name, fn_type, param_names, body);
                            return Err(e)
                        },
                    }),
                    Parameter::Literal(keys) => Data::Literal(keys),
                };
                variables.insert(name.to_string(), data);
            }

            parser.add_func(name, fn_type, param_names, body.clone());

            
            let res = match &body {
                FuncBody::Expression(exp) => Some(BorkInterp::resolve_expression(variables, keyboard, mouse, parser, &exp)?.unwrap_or(0)),
                FuncBody::Literal(coms) => {
                    parser.begin_function(name);
                    let mut res = match BorkInterp::interp_command(variables, keyboard, mouse, parser, if_stack, while_stack, &coms){
                        Ok(res) => res,
                        Err(e) => {
                            parser.end_function();
                            return Err(e)
                        },
                    };
                    while let (true, new_coms) =  res {
                        res = match BorkInterp::interp_command(variables, keyboard, mouse, parser, if_stack, while_stack, &new_coms){
                            Ok(res) => res,
                            Err(e) => {
                                parser.end_function();
                                return Err(e)
                            },
                        };
                    }
                    None
                },
            };

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
            Command::Literal(keys) => for key in keys { BorkInterp::press_key(variables, keyboard, mouse, parser, &key)? },
            Command::Key(keys) => for key in keys { BorkInterp::press_key(variables, keyboard, mouse, parser, &key)? },
            Command::Hold(keys) => for key in keys { BorkInterp::hold_key(variables, keyboard, mouse, parser, &key)? },
            Command::Release(keys) => for key in keys { BorkInterp::release_key(variables, keyboard, mouse, parser, &key)? },
            Command::If(exp) => {
                let cond = BorkInterp::resolve_expression(variables, keyboard, mouse, parser, &exp)?;
                if BorkInterp::resolve_to_bool(cond) == 0 {
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
                if BorkInterp::resolve_to_bool(cond) == 0 {
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
                if BorkInterp::resolve_to_bool(cond) == 0 {
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
            Command::Set(name, param) => {
                match param {
                    Parameter::Expression(exp) => {
                        let value = BorkInterp::resolve_expression(variables, keyboard, mouse, parser, &exp)?;
                        variables.insert(name.to_string(), Data::Integer(value.unwrap_or(0)));
                    },
                    Parameter::Literal(keys) => {
                        variables.insert(name.to_string(), Data::Literal(keys));
                    },
                }
            },
            Command::Expression(exp) => {
                let value = BorkInterp::resolve_expression(variables, keyboard, mouse, parser, &exp)?;
                if let Some(value) = value {
                    keyboard.press_basic_string(&format!("{}", value));
                }
            },
            Command::Move(x, y) => {
                let x = BorkInterp::resolve_expression(variables, keyboard, mouse, parser, &x)?.map(|x|{
                    if x < i8::MIN as i64{
                        i8::MIN
                    } else if x > i8::MAX as i64{
                        i8::MAX
                    } else {
                        x as i8
                    }
                }).unwrap_or(0);

                let y = BorkInterp::resolve_expression(variables, keyboard, mouse, parser, &y)?.map(|y|{
                    if y < i8::MIN as i64{
                        i8::MIN
                    } else if y > i8::MAX as i64{
                        i8::MAX
                    } else {
                        y as i8
                    }
                }).unwrap_or(0);

                mouse.move_mouse(&x, &virt_hid::mouse::MouseDir::X);
                mouse.move_mouse(&y, &virt_hid::mouse::MouseDir::Y);
            },
            Command::Pipe(coms) => {                
                let output = process::Command::new("bash")
                    .args(["-c", coms])
                    .output()
                    .map_err(|e| Error::PipeError(e))?;
                
                keyboard.press_basic_string(&String::from_utf8(output.stdout).map_err(|e| Error::PipeParseError(e))?)
            },
            Command::Call(name, params) => {
                if let Some(value) = BorkInterp::resolve_call(variables, keyboard, mouse, parser, &mut Vec::new(), &mut Vec::new(), DataType::Any, name, params)? {
                    keyboard.press_basic_string(&format!("{}", value));
                }
            },
            Command::None => (),
            Command::LED(state) => {
                keyboard.press_basic_string(&format!("{}", keyboard.led_state(&state)));
            },
            Command::Sleep(exp) => {
                let millis = BorkInterp::resolve_expression(variables, keyboard, mouse, parser, &exp)?;
                if let Some(millis) = millis {
                    if millis > 0 {
                        let duration = Duration::from_millis(millis as u64);
                        thread::sleep(duration);
                    }
                }
            },
            Command::Exit => return Ok((false, new_i)),
        }
        Ok((true, new_i))
    }
}

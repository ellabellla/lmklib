use std::collections::HashMap;

use lmk_hid::key::{SpecialKey, Modifier, LEDState};
use nom::{IResult, bytes::{complete::{tag, take_while, take_while1}}, multi::{fold_many_m_n, many1, separated_list1, many0}, InputIter, InputLength, Slice, AsChar, branch::{alt}, Parser, error::{ErrorKind, Error, ParseError}, sequence::{delimited, preceded, tuple, pair, terminated, separated_pair}, character::{complete::{digit1, space0}}, InputTake};

macro_rules! alt_mut {
    ($a:expr, $($b:expr),+) => {
        {
            let res = $a;
            if let Ok(res) = res {
                Ok(res)
            } $(
                else if let Ok(res) = $b {
                    Ok(res)
                }
            )+ else {
                res
            }
        }
    };
}

#[derive(Debug)]
pub enum Operator<'a>{
    Add(Value<'a>),
    Sub(Value<'a>),
    Mult(Value<'a>),
    Div(Value<'a>),
    Mod(Value<'a>),
    Exp(Value<'a>),

    Equ(Value<'a>),
    NEq(Value<'a>),
    Gre(Value<'a>),
    Les(Value<'a>),
    EqL(Value<'a>),
    EqG(Value<'a>),

    Not(Value<'a>),
    And(Value<'a>),
    Or(Value<'a>),

    BNot(Value<'a>),
    BAnd(Value<'a>),
    BOr(Value<'a>),
    Left(Value<'a>),
    Right(Value<'a>),

    Set(&'a str),
    While(Box<Operator<'a>>),
    If(Box<Expression<'a>>, Box<Expression<'a>>),
}

#[derive(Debug)]
pub enum Value<'a> {
    Int(i64),
    Variable(&'a str),
    Call(&'a str, Vec<Parameter<'a>>),
    Bracket(Box<Expression<'a>>),
    LED(LEDState),
}

#[derive(Debug)]
pub struct Expression<'a> {
    pub value: Value<'a>,
    pub ops: Vec<Operator<'a>>
}

#[derive(Debug)]
pub enum Key<'a> {
    Modifier(Modifier),
    Special(SpecialKey),
    Literal(char),
    ASCII(Expression<'a>),
    Variable(&'a str),
    Keycode(u32),
    Left,
    Right,
    Middle,
}

#[derive(Debug)]
pub enum Parameter<'a> {
    Expression(Expression<'a>),
    Literal(Vec<Key<'a>>),
}

#[derive(Debug)]
pub enum ParameterName<'a> {
    Expression(&'a str),
    Literal(&'a str),
}

#[derive(Debug, Clone, PartialEq)]
pub enum DataType {
    Integer,
    Literal,
    Any,
}

#[derive(Debug)]
pub enum Command <'a> {
    String(Vec<char>),
    Literal(Vec<Key<'a>>),
    Arrow,
    Key(Vec<Key<'a>>),
    Hold(Vec<Key<'a>>),
    Release(Vec<Key<'a>>),
    If(Expression<'a>),
    ElseIf(Expression<'a>),
    Else,
    While(Expression<'a>),
    End,
    Set(&'a str, Expression<'a>),
    Print(&'a str),
    Expression(Expression<'a>),
    Left,
    Right,
    Middle,
    Move(Expression<'a>, Expression<'a>),
    Pipe,
    Call(&'a str, Vec<Parameter<'a>>),
    None,
    LED(LEDState),
    ASCII(Expression<'a>),
}


fn integer<'a>(i: &'a str) -> IResult<&'a str, Value<'a>> {
    let (i, neg) = tag::<&str, &str, Error<&str>>("-")(i)
        .map(|(i, _)| (i, true))
        .unwrap_or((i, false));
    let (i, delay) = digit1(i)?;
    let int: i64 = delay.parse().unwrap();
    Ok((i, Value::Int(if neg {-int} else {int})))
}

fn bool<'a>(i: &'a str) -> IResult<&'a str, Value<'a>> {
    let (i, int) = alt((
        tag("T").map(|_| 1),
        tag("F").map(|_| 0),
    ))(i)?;

    Ok((i, Value::Int(int)))
}

fn bracket<'a>(variables: &HashMap<&'a str, DataType>, functions: &HashMap<&'a str, (DataType, Vec<ParameterName<'a>>, Vec<Command<'a>>)>, i: &'a str) -> IResult<&'a str, Value<'a>> {
    tuple((
        tag("("),
        whitespace0,
        |i| expression(variables, functions, i),
        whitespace0,
        tag(")")
    ))(i).map(|(i, (_,_,expr, _,_))| (i, Value::Bracket(Box::new(expr))))
}

fn variable_name<'a>(i: &'a str) -> IResult<&'a str, &'a str> {
    take_while1(|c: char|  c == '_' || c.is_alphabetic())(i)
}

fn checked_variable_name<'a>(variables: &HashMap<&'a str, DataType>, i_: &'a str) -> IResult<&'a str, &'a str>  {
    let (i, name) = variable_name(i_)?;
    if let Some(variable) = variables.get(name) {
        if matches!(variable, DataType::Integer) {
            return Ok((i, name))
        }
    }
    Err(nom::Err::Error(nom::error::Error::new(i_, ErrorKind::Verify)))
}

fn variable<'a>(variables: &HashMap<&'a str, DataType>, i: &'a str) -> IResult<&'a str, Value<'a>> {
    checked_variable_name(variables, i).map(|(i, name)| (i, Value::Variable(name)))
}

fn variable_escape<'a>(variables: &HashMap<&'a str, DataType>, i_: &'a str) -> IResult<&'a str, Key<'a>> {
    let (i, name) = delimited(tag("\\$"), variable_name, tag("\\"))(i_)?;
    if let Some(variable) = variables.get(name) {
        return Ok((i, Key::Variable(name)))
    }
    Err(nom::Err::Error(nom::error::Error::new(i_, ErrorKind::Verify)))
}

fn led<'a>(i: &'a str) -> IResult<&'a str, LEDState> {
    delimited(
        tag("<&"), 
        alt((
            tag("1").map(|_| LEDState::NumLock), 
            tag("2").map(|_| LEDState::CapsLock), 
            tag("3").map(|_| LEDState::ScrollLock), 
            tag("4").map(|_| LEDState::Compose), 
            tag("5").map(|_| LEDState::Kana),
        )),
        tag(">")
    )(i)
}

fn led_command<'a>(i: &'a str) -> IResult<&'a str, Command> {
    led.map(|l| Command::LED(l)).parse(i)
}


fn ascii<'a>(i_: &'a str) -> IResult<&'a str, Value> {
    let (i, _) = tag("@")(i_)?;
    if let Some(c) = i.chars().next() {
        if c.is_ascii() {
            return Ok((i.split_at(c.len()).1, Value::Int(c as i64)))
        }
    }

    Err(nom::Err::Error(nom::error::Error::new(i_, ErrorKind::Char)))
}

fn value<'a>(variables: &HashMap<&'a str, DataType>, functions: &HashMap<&'a str, (DataType, Vec<ParameterName<'a>>, Vec<Command<'a>>)>, i: &'a str) -> IResult<&'a str, Value<'a>> {
    alt((
        integer,
        bool,
        ascii,
        led.map(|l| Value::LED(l)),
        |i| bracket(variables, functions, i),
        |i| variable(variables, i),
        |i| call_components(DataType::Integer, variables, functions, i).map(|(i,(n, args))| (i, Value::Call(n, args))),
    ))(i)
}

fn binary_operator<'a>(i: &'a str) -> IResult<&'a str, &'a str> {
    alt((
        tag("<<"),
        tag(">>"),

        tag("+"),
        tag("-"),
        tag("*"),
        tag("/"),
        tag("%"),
        tag("^"),

        tag("=="),
        tag("!="),
        tag("<="),
        tag(">="),
        tag(">"),
        tag("<"),

        tag("!"),
        tag("&&"),
        tag("||"),

        tag("~"),
        tag("&"),
        tag("|"),
    ))(i)
}

fn map_binary_operator<'a>(op: &'a str, val: Value<'a>) -> Operator<'a> {
    match op {
        "+" => Operator::Add(val),
        "-" => Operator::Sub(val),
        "*" => Operator::Mult(val),
        "/" => Operator::Div(val),
        "%" => Operator::Mod(val),
        "^" => Operator::Exp(val),

        "==" => Operator::Equ(val),
        "!=" => Operator::NEq(val),
        ">" => Operator::Gre(val),
        "<" => Operator::Les(val),
        "<=" => Operator::EqL(val),
        ">=" => Operator::EqG(val),

        "!" => Operator::Not(val),
        "&&" => Operator::And(val),
        "||" => Operator::Or(val),

        "~" => Operator::BNot(val),
        "&" => Operator::BAnd(val),
        "|" => Operator::BOr(val),
        "<<" => Operator::Left(val),
        ">>" => Operator::Right(val),

        _ => {println!("{}", op) ;unreachable!()},
    }
}

fn expression<'a>(variables: &HashMap<&'a str, DataType>, functions: &HashMap<&'a str, (DataType, Vec<ParameterName<'a>>, Vec<Command<'a>>)>, i: &'a str) -> IResult<&'a str, Expression<'a>> {
    tuple((
        whitespace0,
        |i| value(variables, functions, i),
        many0(alt((
            tuple((
                tag("$"),
                |i| checked_variable_name(variables, i),
            )).map(|(_, name)| Operator::Set(name)),
            tuple((
                tag("?*"),
                |i| expression(variables, functions, i),
                binary_operator
            )).map(|(_, e, op)| Operator::While(Box::new(map_binary_operator(op, Value::Bracket(Box::new(e)))))),
            tuple((
                tag("?"),
                |i| expression(variables, functions, i),
                tag(":"),
                |i| expression(variables, functions, i),
            )).map(|(_, t, _, f)| Operator::If(Box::new(t), Box::new(f))),
            |i| tuple((
                whitespace0,
                binary_operator,
                whitespace0,
                |i| value(variables, functions, i),
                whitespace0,
            )).map(|(_ , op, _, val, _)| {
                (op, val)
            }).parse(i).map(|(i, (op, val))| (i, map_binary_operator(op, val)))
        ))),
        whitespace0
    ))(i).map(|(i, (_, value, ops, _))| (i, Expression{value, ops}))
}

fn character<'a>(i:&'a str) -> IResult<&'a str, char> {
    let mut it = i.iter_indices();
    match it.next() {
        None => Err(nom::Err::Error(nom::error::Error::new(i, ErrorKind::Char))),
        Some((_, c)) => if c != '"' && c != '<' && c != '\\' && c != '\n' && c != '\t' && c != '\r' {
            match it.next() {
                None => Ok((i.slice(i.input_len()..), c.as_char())),
                Some((idx, _)) => Ok((i.slice(idx..), c.as_char())),
            }
        } else {
            Err(nom::Err::Error(nom::error::Error::new(i, ErrorKind::Char)))
        },
    }
}

fn literal<'a>(functions: &HashMap<&'a str, (DataType, Vec<ParameterName<'a>>, Vec<Command<'a>>)>,variables: &HashMap<&'a str, DataType>, i:&'a str) -> IResult<&'a str, Vec<Key<'a>>> {
    delimited(
        tag("\""),
        fold_many_m_n(
            1,
            100,
            alt((
                |i| escape(functions, variables, i),
                tag("\n").map(|_| vec![Key::Special(SpecialKey::Enter)]),
                tag("\t").map(|_| vec![Key::Special(SpecialKey::Tab)]),
                tag("\r").map(|_| vec![]),
                fold_many_m_n(
                1,
                100,
                character.map(|c| Key::Literal(c)),
                Vec::new,
                |mut acc: Vec<_>, item| {
                    acc.push(item);
                    acc
                }),
            )),
            Vec::new,
            |mut acc: Vec<_>, item| {
                acc.extend(item);
                acc
            }
        ),
        tag("\""),
    )(i)
}    

fn characters<'a>(functions: &HashMap<&'a str, (DataType, Vec<ParameterName<'a>>, Vec<Command<'a>>)>, variables: &HashMap<&'a str, DataType>, i:&'a str) -> IResult<&'a str, Command<'a>> {
    fold_many_m_n(
        1,
        100,
        alt((
            |i| escape(functions, variables, i),
            tag("\r").map(|_| vec![]),
            fold_many_m_n(
            1,
            100,
            character.map(|c| Key::Literal(c)),
            Vec::new,
            |mut acc: Vec<_>, item| {
                acc.push(item);
                acc
            }),
        )),
        Vec::new,
        |mut acc: Vec<_>, item| {
            acc.extend(item);
            acc
        }
    ).map(|k| Command::Literal(k)).parse(i)
}

fn modifier<'a>(i: &'a str) -> IResult<&'a str, Modifier> {
    alt((
        tag("ALT").map(|_| Modifier::LeftAlt),
        tag("CTL").map(|_| Modifier::LeftControl),
        tag("CONTROL").map(|_| Modifier::LeftControl),
        tag("COMMAND").map(|_| Modifier::LeftMeta),
        tag("GUI").map(|_| Modifier::LeftMeta),
        tag("SHIFT").map(|_| Modifier::LeftShift),
    ))(i)
}

fn special<'a>(i: &'a str) -> IResult<&'a str, SpecialKey> {
    alt((
        alt((
            tag("UP").map(|_| SpecialKey::UpArrow),
            tag("DOWN").map(|_| SpecialKey::DownArrow),
            tag("LEFT").map(|_| SpecialKey::LeftArrow),
            tag("RIGHT").map(|_| SpecialKey::RightArrow),
            tag("UPARROW").map(|_| SpecialKey::UpArrow),
            tag("DOWNARROW").map(|_| SpecialKey::DownArrow),
            tag("LEFTARROW").map(|_| SpecialKey::LeftArrow),
            tag("RIGHTARROW").map(|_| SpecialKey::RightArrow),
            tag("PAGEUP").map(|_| SpecialKey::PageUp),
            tag("PAGEDOWN").map(|_| SpecialKey::PageDown),
            tag("INSERT").map(|_| SpecialKey::Insert),
            tag("DELETE").map(|_| SpecialKey::DeleteForward),
            tag("DEL").map(|_| SpecialKey::DeleteForward),
            tag("CAPSLOCK").map(|_| SpecialKey::CapsLock),
            tag("NUMLOCK").map(|_| SpecialKey::NumLockAndClear),
            tag("SCROLLOCK").map(|_| SpecialKey::ScrollLock),
        )),
        tag("BACKSPACE").map(|_| SpecialKey::Backspace),
        tag("TAB").map(|_| SpecialKey::Tab),
        tag("SPACE").map(|_| SpecialKey::Space),
        tag("F1").map(|_| SpecialKey::F1),
        tag("F2").map(|_| SpecialKey::F2),
        tag("F3").map(|_| SpecialKey::F3),
        tag("F4").map(|_| SpecialKey::F4),
        tag("F5").map(|_| SpecialKey::F5),
        tag("F6").map(|_| SpecialKey::F6),
        tag("F7").map(|_| SpecialKey::F7),
        tag("F8").map(|_| SpecialKey::F8),
        tag("F9").map(|_| SpecialKey::F9),
        tag("F10").map(|_| SpecialKey::F10),
        tag("F11").map(|_| SpecialKey::F11),
        tag("F12").map(|_| SpecialKey::F12),
        tag("ENTER").map(|_| SpecialKey::Enter),
        tag("ESCAPE").map(|_| SpecialKey::Escape),
        tag("PAUSEBREAK").map(|_| SpecialKey::Pause),
        tag("PRINTSCREEN").map(|_| SpecialKey::PrintScreen),
        tag("MENUAPP").map(|_| SpecialKey::Menu),
    ))(i)
}

fn key<'a>(functions: &HashMap<&'a str, (DataType, Vec<ParameterName<'a>>, Vec<Command<'a>>)>, variables: &HashMap<&'a str, DataType>, i:&'a str) -> IResult<&'a str, Command<'a>> {
    delimited(
        tag("<"),
        separated_list1(
            tag(";"),
            alt((
                special.map(|s| vec![Key::Special(s)]),
                modifier.map(|m| vec![Key::Modifier(m)]),
                |i| literal(functions, variables, i)
            ))
        ),
        tag(">"),
    )(i).map(|(i,k)| (i, Command::Key(k.into_iter().flatten().collect())))
}

fn hold<'a>(functions: &HashMap<&'a str, (DataType, Vec<ParameterName<'a>>, Vec<Command<'a>>)>, variables: &HashMap<&'a str, DataType>, i:&'a str) -> IResult<&'a str, Command<'a>> {
    terminated(
        pair(
            alt((tag("<-"),tag("<_"))),
            separated_list1(
                tag(";"),
                alt((
                    special.map(|s| vec![Key::Special(s)]),
                    modifier.map(|m| vec![Key::Modifier(m)]),
                    |i| literal(functions, variables, i)
                ))
            ),
        ),
        tag(">"),
    )(i)
    .map(|(i, (t, k))| (i, (t, k.into_iter().flatten().collect())))
    .map(|(i,(t, k))| (i, if t == "<-" {Command::Release(k)} else {Command::Hold(k)}))
}

fn if_<'a>(variables: &HashMap<&'a str, DataType>, functions: &HashMap<&'a str, (DataType, Vec<ParameterName<'a>>, Vec<Command<'a>>)>, nest_stack: &mut Vec<NestType>, i:&'a str) -> IResult<&'a str, Command<'a>> {
    delimited(
        tag("<?"),
        |i| expression(variables, functions, i),
        tag(";")
    ).map(|e|  Command::If(e))
    .parse(i)
    .and_then(|res| {
        nest_stack.push(NestType::If);
        Ok(res)
    })
}

fn if_else<'a>(variables: &HashMap<&'a str, DataType>, functions: &HashMap<&'a str, (DataType, Vec<ParameterName<'a>>, Vec<Command<'a>>)>, nest_stack: &Vec<NestType>, i: &'a str) -> IResult<&'a str, Command<'a>> {
    if matches!(nest_stack.last(), Some(NestType::If)) {
        delimited(
                    tag(";?"),
                    |i| expression(variables, functions, i),
                    tag(";")
        ).map(|e| Command::ElseIf(e)).parse(i)
    } else {
        Err(nom::Err::Error(Error::from_error_kind(i, ErrorKind::Char)))
    }
}

fn else_<'a>(nest_stack: &Vec<NestType>, i: &'a str) -> IResult<&'a str, Command<'a>> {
    if matches!(nest_stack.last(), Some(NestType::If)) {
        tag(";")
        .map(|_| Command::Else).parse(i)
    } else {
        Err(nom::Err::Error(Error::from_error_kind(i, ErrorKind::Char)))
    }
}

fn while_<'a>(variables: &HashMap<&'a str, DataType>, functions: &HashMap<&'a str, (DataType, Vec<ParameterName<'a>>, Vec<Command<'a>>)>, nest_stack: &mut Vec<NestType>, i:&'a str) -> IResult<&'a str, Command<'a>> {
    delimited(
        tag("<*"),
        |i| expression(variables, functions, i),
        tag(";")
    )
    .map(|e| {
        Command::While(e)
    }).parse(i)
    .and_then(|res| {
        nest_stack.push(NestType::While);
        Ok(res)
    })
}

fn end<'a>(nest_stack: &mut Vec<NestType>, i: &'a str) -> IResult<&'a str, Command<'a>> {
    if let Some(last) = nest_stack.last() {
        if !matches!(last, NestType::Function) {
            return tag(">").map(|_| Command::End).parse(i)
            .and_then(|res| {
                nest_stack.pop();
                Ok(res)
            })
        }
    }
    
    Err(nom::Err::Error(Error::from_error_kind(i, ErrorKind::Char)))
    
}

fn set<'a>(variables: &mut HashMap<&'a str, DataType>, functions: &HashMap<&'a str, (DataType, Vec<ParameterName<'a>>, Vec<Command<'a>>)>, i_:&'a str) -> IResult<&'a str, Command<'a>> {
    let (i, (name, exp)) = delimited(
        tag("<="), 
        separated_pair(
            variable_name, 
            tag(";"),
            |i| expression(variables, functions, i)
        ),
        tag(">")
    )(i_)?;

    if let Some(dtype) = variables.get(name) {
        if !matches!(dtype, DataType::Integer) {
            return Err(nom::Err::Error(Error::from_error_kind(i_, ErrorKind::Verify)))
        }
    } else {
        variables.insert(name, DataType::Integer);
    }

    Ok((i, Command::Set(name, exp)))
    
}

fn print<'a>(variables: &mut HashMap<&'a str, DataType>, i_:&'a str) -> IResult<&'a str, Command<'a>> {
    let (i, name) = delimited(
        tag("<$"), 
        variable_name,
        tag(">")
    )(i_)?;

    if let Some(_) = variables.get(name) {
        Ok((i, Command::Print(name)))
    } else {
        Err(nom::Err::Error(Error::from_error_kind(i_, ErrorKind::Verify)))
    }
}

fn exp<'a>(variables: &HashMap<&'a str, DataType>, functions: &HashMap<&'a str, (DataType, Vec<ParameterName<'a>>, Vec<Command<'a>>)>, i:&'a str) -> IResult<&'a str, Command<'a>> { 
    delimited(
        tag("<#"),
        |i| expression(variables, functions, i),
        tag(">")
    ).map(|e| Command::Expression(e))
    .parse(i)
}

fn ascii_escape<'a>(variables: &HashMap<&'a str, DataType>, functions: &HashMap<&'a str, (DataType, Vec<ParameterName<'a>>, Vec<Command<'a>>)>, i:&'a str) -> IResult<&'a str, Key<'a>> { 
    delimited(
        tag("\\@"),
        |i| expression(variables, functions, i),
        tag("\\")
    ).map(|e| Key::ASCII(e))
    .parse(i)
}

fn ascii_command<'a>(variables: &HashMap<&'a str, DataType>, functions: &HashMap<&'a str, (DataType, Vec<ParameterName<'a>>, Vec<Command<'a>>)>, i:&'a str) -> IResult<&'a str, Command<'a>> { 
    delimited(
        tag("<@"),
        |i| expression(variables, functions, i),
        tag(">")
    ).map(|e| Command::ASCII(e))
    .parse(i)
}

fn click<'a>(i:&'a str) -> IResult<&'a str, Command> { 
    delimited(
        tag("<^"), 
        alt((
            tag("1").map(|_| Command::Left),
            tag("2").map(|_| Command::Right),
            tag("3").map(|_| Command::Middle),
        )), 
        tag(">")
    )(i)
}

fn move_<'a>(variables: &HashMap<&'a str, DataType>, functions: &HashMap<&'a str, (DataType, Vec<ParameterName<'a>>, Vec<Command<'a>>)>, i:&'a str) -> IResult<&'a str, Command<'a>> {
    delimited(
        tag("<%"), 
        separated_pair(
            |i| expression(variables, functions, i), 
            tag(";"),
            |i| expression(variables, functions, i)
        ),
        tag(">")
    )
    .map(|(x, y)| Command::Move(x, y)).parse(i)
}

fn pipe<'a>(nest_stack: &mut Vec<NestType>,i:&'a str) -> IResult<&'a str, Command<'a>> {
    tag("<|").map(|_| Command::Pipe).parse(i)
    .and_then(|res| {
        nest_stack.push(NestType::Pipe);
        Ok(res)
    })
}

fn call_components<'a>(expected: DataType, variables: &HashMap<&'a str, DataType>, functions: &HashMap<&'a str, (DataType, Vec<ParameterName<'a>>, Vec<Command<'a>>)>, i_:&'a str) -> IResult<&'a str, (&'a str, Vec<Parameter<'a>>)> {
    let (i, name) = preceded(
        tag("<!"),         
    variable_name
    )(i_)?;

    if let Some((fn_type, params, _)) = functions.get(name) {
        if expected != DataType::Any && *fn_type != expected {
            return Err(nom::Err::Error(nom::error::Error::new(i_, ErrorKind::Verify)))
        }

        let mut params_res = Vec::with_capacity(params.len());
        let mut i = i;
        for param in params {
            let (new_i, param) = preceded(
                tag(";"),
                |i| match param {
                    ParameterName::Expression(_) => expression(variables, functions, i).map(|(i, e)| (i, Parameter::Expression(e))),
                    ParameterName::Literal(_) => literal(functions, variables, i).map(|(i, l)| (i, Parameter::Literal(l))),
                },
            )(i)?;
            i = new_i;
            params_res.push(param);
        }

        let (i, _) = tag(">")(i)?;
        Ok((i, (name, params_res)))
    } else {
        return Err(nom::Err::Error(Error::from_error_kind(i_, ErrorKind::Verify)))
    }   
}

fn call<'a>(variables: &HashMap<&'a str, DataType>, functions: &HashMap<&'a str, (DataType, Vec<ParameterName<'a>>, Vec<Command<'a>>)>, i: &'a str) -> IResult<&'a str, Command<'a>> {
    call_components(DataType::Any, variables, functions, i).map(|(i, (name, params))| (i, Command::Call(name, params)))
}

fn hex_digit<'a>(i:&'a str) -> IResult<&'a str, u32> {
    alt((
        alt((
            tag("0").map(|_| 0),
            tag("1").map(|_| 1),
            tag("2").map(|_| 2),
            tag("3").map(|_| 3),
            tag("4").map(|_| 4),
            tag("5").map(|_| 5),
            tag("6").map(|_| 6),
            tag("7").map(|_| 7),
            tag("8").map(|_| 8),
            tag("9").map(|_| 9),
            tag("a").map(|_| 10),
            tag("b").map(|_| 11),
        )),
        alt((
            tag("c").map(|_| 12),
            tag("d").map(|_| 13),
            tag("e").map(|_| 14),
            tag("f").map(|_| 15),
            tag("A").map(|_| 10),
            tag("B").map(|_| 11),
            tag("C").map(|_| 12),
            tag("D").map(|_| 13),
            tag("E").map(|_| 14),
            tag("F").map(|_| 15),
        ))
    ))(i)
}

fn hex<'a>(i:&'a str) -> IResult<&'a str, u32> {
    pair(
        hex_digit,
        hex_digit
    ).map(|(a, b)| a * 16 + b).parse(i)
}   

fn escape<'a>(functions: &HashMap<&'a str, (DataType, Vec<ParameterName<'a>>, Vec<Command<'a>>)>, variables: &HashMap<&'a str, DataType>, i:&'a str) -> IResult<&'a str, Vec<Key<'a>>> {
    many1(alt((
        |i| variable_escape(variables, i),
        |i| ascii_escape(variables, functions, i),
        tag("\\n").map(|_| Key::Special(SpecialKey::Enter)),
        tag("\\t").map(|_| Key::Special(SpecialKey::Tab)),
        tag("\\b").map(|_| Key::Special(SpecialKey::Backspace)),
        tag("\\l").map(|_| Key::Left),
        tag("\\r").map(|_| Key::Right),
        tag("\\m").map(|_| Key::Middle),
        tag("\\\"").map(|_| Key::Literal('\"')),
        tag("\\\\").map(|_| Key::Literal('\\')),
        tag("\\<").map(|_| Key::Literal('<')),
        preceded(
            tag("\\x"),
            hex
        ).map(|h| Key::Keycode(h))
    )))(i)
}

fn inside_pipe<'a, 'b>(i:&'a str) -> IResult<&'a str, Command<'a>> {
    fold_many_m_n(
        1, 100,
        alt((
            tag("\\<").map(|_| vec!['<']),
            tag("\\>").map(|_| vec!['>']),
            tag("\\\\").map(|_| vec!['\\']),
            take_while1(|c:char| c != '<' && c != '>' && c != '\\').map(|c: &'a str| c.chars().collect()),
        )),
        || Vec::new(), 
        |mut a: Vec<char>, i| {
            a.extend(i);
            a
        }
    ).map(|c: Vec<char>| Command::String(c)).parse(i)
}

fn command<'a>(variables: &mut HashMap<&'a str, DataType>, functions: &mut HashMap<&'a str, (DataType, Vec<ParameterName<'a>>, Vec<Command<'a>>)>, nest_stack: &mut Vec<NestType>, i:&'a str) -> IResult<&'a str, Command<'a>> {
    delimited(
        take_while(|c: char| c != ' ' && c.is_whitespace()),
        |i| alt_mut!(
            literal(functions, variables, i).map(|(i, l)| (i, Command::Literal(l))),
            escape(functions, variables, i).map(|(i, k)| (i, Command::Key(k))),
            if_else(variables, functions, nest_stack, i),
            else_(nest_stack, i),
            key(functions, variables, i),
            hold(functions, variables, i),
            set(variables, functions, i),
            print(variables, i),
            exp(variables, functions, i),
            click(i),
            move_(variables, functions, i),
            call(variables, functions, i),
            if_(variables, functions, nest_stack, i),
            while_(variables, functions, nest_stack, i),
            pipe(nest_stack, i),
            end(nest_stack, i),
            led_command(i),
            ascii_command(variables, functions, i),
            function(variables, functions, nest_stack, i)
        ),
        take_while(|c: char| c != ' ' && c.is_whitespace())
    )(i)
}

fn function<'a>(variables: &mut HashMap<&'a str, DataType>, functions: & mut HashMap<&'a str, (DataType, Vec<ParameterName<'a>>, Vec<Command<'a>>)>, nest_stack: &mut Vec<NestType>, i_:&'a str) -> IResult<&'a str, Command<'a>> {
    if nest_stack.len() > 0 {
        return Err(nom::Err::Error(nom::error::Error::from_error_kind(i_, ErrorKind::Verify)))
    }

    nest_stack.push(NestType::Function);
    
    let res = preceded(
        tag("<+"), 
        tuple((
            alt((tag("\""), tag("#"))),
            terminated(
                variable_name,
                tag(";"),
            ),
            many0(
                terminated(
                    alt((
                        preceded(tag("#"), variable_name).map(|n| ParameterName::Expression(n)),
                        preceded(tag("\""), variable_name).map(|n| ParameterName::Literal(n)),
                    )),
                    tag(";"),
                )
            )
        )),
    )(i_);
    let (i, (fn_type, name, params)) = if let Ok(res) = res  {
        res
    } else {
        nest_stack.pop();
        return Err(res.unwrap_err())
    };
    let fn_type = if fn_type == "#" {DataType::Integer} else {DataType::Literal};

    for param in &params {
        let (n, dtype) = match param {
            ParameterName::Expression(n) => (n, DataType::Integer),
            ParameterName::Literal(n) => (n, DataType::Literal),
        };

        if variables.contains_key(n) {
            return Err(nom::Err::Error(Error::from_error_kind(i_, ErrorKind::Verify)))
        }

        variables.insert(n, dtype);
    }

    functions.insert(name, (fn_type.clone(), params, vec![]));

    let res = terminated(
            |i| match fn_type {
                DataType::Literal => many1(delimited(whitespace0, |i| command(variables, functions, nest_stack, i), whitespace0))(i),
                DataType::Integer => expression(variables, functions, i)
                    .map(|(i, e)| (i, vec![Command::Expression(e)])),
                _ => unreachable!()
            },
        preceded(whitespace0, tag(">"))
    )(i);

    let (i, coms) = if let Ok(res) = res  {
        res
    } else {
        nest_stack.pop();
        functions.remove(name);
        return Err(res.unwrap_err())
    };

    let (fn_type, params, _) = functions.remove(name).expect("function removed whilst being created");

    for param in &params {
        let n = match param {
            ParameterName::Expression(n) => n,
            ParameterName::Literal(n) => n,
        };

        variables.remove(n);
    }

    nest_stack.pop();
    

    if functions.contains_key(name) {
        return Err(nom::Err::Error(nom::error::Error::from_error_kind(i_, ErrorKind::Verify)))
    }

    functions.insert(name, (fn_type, params, coms));
    Ok((i, Command::None))
}

fn whitespace0<'a>(i: &'a str) -> IResult<&'a str, ()> {
    take_while(|c: char| c.is_whitespace())
    .map(|a| ()).parse(i)
}

enum NestType {
    While,
    If,
    Pipe,
    Function,
}

pub struct BorkParser<'a> {
    nest_stack: Vec<NestType>,
    functions: HashMap<&'a str, (DataType, Vec<ParameterName<'a>>, Vec<Command<'a>>)>,
    variables: HashMap<&'a str, DataType>,
}


impl<'a> BorkParser<'a> {
    pub fn new() -> BorkParser<'a> {
        BorkParser { nest_stack: Vec::new(), functions: HashMap::new(), variables: HashMap::new() }
    }
    pub fn parse_command(&mut self, i:&'a str) -> IResult<&'a str, Command<'a>> {
        delimited(
            take_while(|c| c == '\n'),
            |i| command(&mut self.variables, &mut self.functions, &mut self.nest_stack, i).or_else(|_| 
            if matches!(self.nest_stack.last(), Some(NestType::Pipe)) {
                inside_pipe(i)
            } else {
                characters(&self.functions, &self.variables, i)
            }),
            take_while(|c| c == '\n'),
        )(i)
    }
}


#[cfg(test)]
mod tests {
    use lmk_hid::key::{SpecialKey, Modifier, LEDState};
    use crate::parser::{Command, Key, Value, Operator, Parameter, function};

    use super::BorkParser;


    #[test]
    fn test() {
        let mut borker = BorkParser::new();

        let (_, com) = borker.parse_command("cajscbakhb\\<\\n\\x00<").unwrap();
        if let Command::Literal(str) = com {
            for (key, expected) in str.iter().zip("cajscbakhb".chars()) {
                if let Key::Literal(c) = key {
                    if *c == expected {
                        continue;
                    }
                }
                assert!(false);
            }
        } else {
            assert!(false);
        }

        if let Command::Key(s) = borker.parse_command("\\@97\\").unwrap().1 {
            assert_eq!(s.len(), 1);
            if let Key::ASCII(e) = &s[0] {
                assert_eq!(e.ops.len(), 0);
                assert!(matches!(e.value, Value::Int(97)))
            } else {
                assert!(false);
            }
        } else {
            assert!(false);
        }

        if let Command::Key(keys) = borker.parse_command("<BACKSPACE;GUI;\"x\\\\ \\\"\">").unwrap().1 {
            assert_eq!(keys.len(), 6);
            assert!(matches!(keys[0], Key::Special(SpecialKey::Backspace)));
            assert!(matches!(keys[1], Key::Modifier(Modifier::LeftMeta)));
            assert!(matches!(keys[2], Key::Literal('x')));
            assert!(matches!(keys[3], Key::Literal('\\')));
            assert!(matches!(keys[4], Key::Literal(' ')));
            assert!(matches!(keys[5], Key::Literal('"')));
        } else {
            assert!(false);
        }

        if let Command::Hold(keys) = borker.parse_command("<_BACKSPACE;GUI;\"x\\\\ \\\"\">").unwrap().1 {
            assert_eq!(keys.len(), 6);
            assert!(matches!(keys[0], Key::Special(SpecialKey::Backspace)));
            assert!(matches!(keys[1], Key::Modifier(Modifier::LeftMeta)));
            assert!(matches!(keys[2], Key::Literal('x')));
            assert!(matches!(keys[3], Key::Literal('\\')));
            assert!(matches!(keys[4], Key::Literal(' ')));
            assert!(matches!(keys[5], Key::Literal('"')));
        } else {
            assert!(false);
        }

        if let Command::Release(keys) = borker.parse_command("<-BACKSPACE;GUI;\"x\\\\ \\\"\">").unwrap().1 {
            assert_eq!(keys.len(), 6);
            assert!(matches!(keys[0], Key::Special(SpecialKey::Backspace)));
            assert!(matches!(keys[1], Key::Modifier(Modifier::LeftMeta)));
            assert!(matches!(keys[2], Key::Literal('x')));
            assert!(matches!(keys[3], Key::Literal('\\')));
            assert!(matches!(keys[4], Key::Literal(' ')));
            assert!(matches!(keys[5], Key::Literal('"')));
        } else {
            assert!(false);
        }

        let (i, com) = borker.parse_command("<?10>1;;?10<1;;>").unwrap();
        assert!(matches!(com, Command::If(_a)));
        let (i, com) = borker.parse_command(i).unwrap();
        assert!(matches!(com, Command::ElseIf(_a)));
        let (i, com) = borker.parse_command(i).unwrap();
        assert!(matches!(com, Command::Else));
        let (_, com) = borker.parse_command(i).unwrap();
        assert!(matches!(com, Command::End));

        let (i, com) = borker.parse_command("<*10>1;>").unwrap();
        assert!(matches!(com, Command::While(_a)));
        let (_, com) = borker.parse_command(i).unwrap();
        assert!(matches!(com, Command::End));


        let (_, com) = borker.parse_command(">").unwrap();
        assert!(matches!(com, Command::Literal(_a)));

        if let Command::Set(name, exp) = borker.parse_command("<=x;10>").unwrap().1 {
            assert_eq!(name, "x");
            assert!(matches!(exp.value, Value::Int(10)));
        } else {
            assert!(false);
        }

        if let Command::Key(s) = borker.parse_command("\\$x\\").unwrap().1 {
            assert_eq!(s.len(), 1);
            assert!(matches!(s[0], Key::Variable("x")));
        } else {
            assert!(false);
        }

        assert!(matches!(borker.parse_command("<$x>").unwrap().1, Command::Print("x")));

        if let Command::Expression(exp) = borker.parse_command("<#x*10>").unwrap().1 {
            assert!(matches!(exp.value, Value::Variable("x")));
            assert_eq!(exp.ops.len(), 1);
            assert!(matches!(exp.ops[0], Operator::Mult(Value::Int(10))))
        } else {
            assert!(false);
        }

        if let Command::Expression(mut exp) = borker.parse_command("<#x?*10+>").unwrap().1 {
            assert!(matches!(exp.value, Value::Variable("x")));
            assert_eq!(exp.ops.len(), 1);
            if let Operator::While(op) = exp.ops.remove(0) {
                if let Operator::Add(Value::Bracket(e)) = *op {
                    assert_eq!(e.ops.len(), 0);
                    assert!(matches!(e.value, Value::Int(10)))
                } else {
                    assert!(false)
                }
            }else {
                assert!(false)
            }
        } else {
            assert!(false);
        }

        if let Command::Expression(mut exp) = borker.parse_command("<#x?1:0>").unwrap().1 {
            assert!(matches!(exp.value, Value::Variable("x")));
            assert_eq!(exp.ops.len(), 1);
            if let Operator::If(t, f) = exp.ops.remove(0) {
                assert_eq!(t.ops.len(), 0);
                assert!(matches!(t.value, Value::Int(1)));

                assert_eq!(f.ops.len(), 0);
                assert!(matches!(f.value, Value::Int(0)));
            }else {
                assert!(false)
            }
        } else {
            assert!(false);
        }

        if let Command::Expression(exp) = borker.parse_command("<#10$x>").unwrap().1 {
            assert!(matches!(exp.value, Value::Int(10)));
            assert_eq!(exp.ops.len(), 1);
            assert!(matches!(exp.ops[0], Operator::Set("x")))
        } else {
            assert!(false);
        }

        if let Command::Expression(exp) = borker.parse_command("<#<&1>>").unwrap().1 {
            assert!(matches!(exp.value, Value::LED(LEDState::NumLock)));
            assert_eq!(exp.ops.len(), 0);
        } else {
            assert!(false);
        }

        if let Command::Expression(exp) = borker.parse_command("<#@a>").unwrap().1 {
            assert!(matches!(exp.value, Value::Int(97)));
            assert_eq!(exp.ops.len(), 0);
        } else {
            assert!(false);
        }

        if let Command::ASCII(exp) = borker.parse_command("<@@a>").unwrap().1 {
            assert!(matches!(exp.value, Value::Int(97)));
            assert_eq!(exp.ops.len(), 0);
        } else {
            assert!(false);
        }

        assert!(matches!(borker.parse_command("<^1>"), Ok((_, Command::Left))));
        assert!(matches!(borker.parse_command("<^2>"), Ok((_, Command::Right))));
        assert!(matches!(borker.parse_command("<^3>"), Ok((_, Command::Middle))));

        if let Command::Move(x, y) = borker.parse_command("<%10;-20>").unwrap().1 {
            assert!(matches!(x.value, Value::Int(10)));
            assert!(matches!(y.value, Value::Int(-20)));
        } else {
            assert!(false);
        }

        let (i, com) = borker.parse_command("<|ls /dev/loop\\<<#10>>").unwrap();
        assert!(matches!(com, Command::Pipe));
        let (i, com) = borker.parse_command(i).unwrap();
        if let Command::String(str) = com {
            assert_eq!(str.iter().collect::<String>(), "ls /dev/loop<");
        } else {
            assert!(false);
        }
        let (i, com) = borker.parse_command(i).unwrap();
        if let Command::Expression(exp) = com {
            assert!(matches!(exp.value, Value::Int(10)));
        } else {
            assert!(false);
        }

        let  (_, com) = borker.parse_command(i).unwrap();
        assert!(matches!(com, Command::End));


        assert!(matches!(function(&mut borker.variables, &mut borker.functions, &mut borker.nest_stack, "<+\"hello;\"name;\"Hello my name is\"<$name>>").unwrap(), ("", Command::None)));
        if let (_, Command::Call("hello", params)) = borker.parse_command("<!hello;\"ella\">").unwrap() {
            assert_eq!(params.len(), 1);
            if let Parameter::Literal(s) = &params[0] {
                assert_eq!(s.len(), 4);
                assert!(matches!(s[0], Key::Literal('e')));
                assert!(matches!(s[1], Key::Literal('l')));
                assert!(matches!(s[2], Key::Literal('l')));
                assert!(matches!(s[3], Key::Literal('a')));
            } else {
                assert!(false);
            }
        } else {
            assert!(false);
        }

        assert!(matches!(function(&mut borker.variables, &mut borker.functions, &mut borker.nest_stack, "<+#add;#a;#b;a+b>").unwrap(), ("", Command::None)));
        if let (_, Command::Call("add", params)) = borker.parse_command("<!add;10;1>").unwrap() {
            assert_eq!(params.len(), 2);
            if let Parameter::Expression(e) = &params[0] {
                assert!(matches!(e.value, Value::Int(10)));
            } else {
                assert!(false);
            }
            if let Parameter::Expression(e) = &params[1] {
                assert!(matches!(e.value, Value::Int(1)));
            } else {
                assert!(false);
            }
        } else {
            assert!(false);
        }
    }


    #[test]
    fn fib() {
        let mut borker = BorkParser::new();
        borker.parse_command(r#"
<+#fib;#x;
    x <= 1 ? 
        x < 1 ? 0 : 1
    :
        <!fib;x-1> + <!fib;x-2>
>"#).unwrap();
    }

    #[test]
    fn greeting() {
        let mut borker = BorkParser::new();
        borker.parse_command(r#"
Hello <|echo $USER>,\n
\n
How are you doing today?
"#).unwrap();
    }
}
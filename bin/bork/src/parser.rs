use std::collections::HashMap;

use virt_hid::key::{SpecialKey, Modifier, LEDState};
use nom::{IResult, bytes::{complete::{tag, take_while, take_while1}}, multi::{fold_many_m_n, many1, separated_list1, many0}, InputIter, InputLength, Slice, AsChar, branch::{alt}, Parser, error::{ErrorKind, Error, ParseError}, sequence::{delimited, preceded, tuple, pair, terminated, separated_pair}, character::{complete::{digit1}}, InputTake, combinator::eof};

macro_rules! tuple_mut_inner {
    ($i:ident, $a:expr) => {
        {
            let (new_i, res) =  $a($i)?;
            $i = new_i;
            res
        }
    };
}

macro_rules! tuple_mut {
    ($i:ident, $a:expr, $($b:expr),+) => {
        {
            let mut $i = $i;
            Some(({
                let (new_i, res) =  $a($i)?;
                $i = new_i;
                res
            } $(, tuple_mut_inner!($i, $b))+)
            ).map(|tup| ($i, tup))
        }
    };
}

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

#[derive(Debug, Clone)]
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

    And(Value<'a>),
    Or(Value<'a>),

    BAnd(Value<'a>),
    BOr(Value<'a>),
    Left(Value<'a>),
    Right(Value<'a>),

    Set(&'a str),
    While(Box<Expression<'a>>, Box<Operator<'a>>),
    If(Box<Expression<'a>>, Box<Expression<'a>>),
}

#[derive(Debug, Clone)]
pub enum Value<'a> {
    Int(i64),
    Variable(&'a str),
    Call(&'a str, Vec<Parameter<'a>>),
    Bracket(Box<Expression<'a>>),
    LED(LEDState),
    BNot(Box<Expression<'a>>),
    Not(Box<Expression<'a>>),
}

#[derive(Debug, Clone)]
pub struct Expression<'a> {
    pub value: Value<'a>,
    pub ops: Vec<Operator<'a>>
}

#[derive(Debug, Clone)]
pub enum Key<'a> {
    Modifier(Modifier),
    Special(SpecialKey),
    Literal(char),
    ASCII(Expression<'a>),
    Variable(&'a str),
    Keycode(u8),
    Left,
    Right,
    Middle,
}

#[derive(Debug, Clone)]
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
    Literal(Vec<Key<'a>>),
    Key(Vec<Key<'a>>),
    Hold(Vec<Key<'a>>),
    Release(Vec<Key<'a>>),
    If(Expression<'a>),
    ElseIf(Expression<'a>),
    Else,
    While(Expression<'a>),
    End,
    Set(&'a str, Parameter<'a>),
    Expression(Expression<'a>),
    Move(Expression<'a>, Expression<'a>),
    Pipe(&'a str),
    Call(&'a str, Vec<Parameter<'a>>),
    None,
    LED(LEDState),
    Exit,
    Sleep(Expression<'a>),
}

fn integer<'a>(i: &'a str) -> IResult<&'a str, i64> {
    let (i, neg) = tag::<&str, &str, Error<&str>>("-")(i)
        .map(|(i, _)| (i, true))
        .unwrap_or((i, false));
    let (i, delay) = digit1(i)?;
    let int: i64 = delay.parse().unwrap();
    Ok((i, if neg {-int} else {int}))
}


fn integer_value<'a>(i: &'a str) -> IResult<&'a str, Value<'a>> {
    integer.map(|i| Value::Int(i)).parse(i)
}

fn bool<'a>(i: &'a str) -> IResult<&'a str, Value<'a>> {
    let (i, int) = alt((
        tag("T").map(|_| 1),
        tag("F").map(|_| 0),
    ))(i)?;

    Ok((i, Value::Int(int)))
}

fn bracket<'a>(variables: &mut HashMap<&'a str, DataType>, functions: & HashMap<&'a str, (DataType, Vec<ParameterName<'a>>, FuncBody<'a>)>, i: &'a str) -> IResult<&'a str, Value<'a>> {
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

fn checked_variable_name<'a>(variables: &HashMap<&'a str, DataType>, i_: &'a str) -> IResult<&'a str, (&'a str, DataType)>  {
    let (i, name) = variable_name(i_)?;
    if let Some(variable) = variables.get(name) {
        return Ok((i, (name, variable.clone())))
    }
    Err(nom::Err::Error(nom::error::Error::new(i_, ErrorKind::Verify)))
}

fn variable<'a>(variables: &HashMap<&'a str, DataType>, i_: &'a str) -> IResult<&'a str, Value<'a>> {
    let (i, (name, data_type)) =  checked_variable_name(variables, i_)?;
    if matches!(data_type, DataType::Integer) {
        Ok((i, Value::Variable(name)))
    } else {
        Err(nom::Err::Error(nom::error::Error::new(i_, ErrorKind::Verify)))
    }

}

fn variable_escape<'a>(variables: &HashMap<&'a str, DataType>, i_: &'a str) -> IResult<&'a str, Key<'a>> {
    let (i, name) = delimited(tag("\\$"), variable_name, tag("\\"))(i_)?;
    if let Some(_) = variables.get(name) {
        return Ok((i, Key::Variable(name)))
    }
    Err(nom::Err::Error(nom::error::Error::new(i_, ErrorKind::Verify)))
}

fn led<'a>(i: &'a str) -> IResult<&'a str, LEDState> {
    preceded(
        tag("\\&"), 
        alt((
            tag("1").map(|_| LEDState::NumLock), 
            tag("2").map(|_| LEDState::CapsLock), 
            tag("3").map(|_| LEDState::ScrollLock), 
            tag("4").map(|_| LEDState::Compose), 
            tag("5").map(|_| LEDState::Kana),
        )),
    )(i)
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

fn not_value<'a>(variables: &mut HashMap<&'a str, DataType>, functions: & HashMap<&'a str, (DataType, Vec<ParameterName<'a>>, FuncBody<'a>)>, i: &'a str) -> IResult<&'a str, Value<'a>> {
    tuple((
        alt((tag("~"), tag("!"))),
        |i| expression(variables, functions, i)
    )).map(|(t, exp)| if t == "!" {Value::BNot(Box::new(exp))} else {Value::Not(Box::new(exp))}).parse(i)
}

fn value<'a>(variables: &mut HashMap<&'a str, DataType>, functions: & HashMap<&'a str, (DataType, Vec<ParameterName<'a>>, FuncBody<'a>)>, i: &'a str) -> IResult<&'a str, Value<'a>> {
    alt_mut!(
        integer_value(i),
        bool(i),
        ascii(i),
        not_value(variables, functions, i),
        led.map(|l| Value::LED(l)).parse(i),
        bracket(variables, functions, i),
        variable(variables, i),
        call_components(DataType::Integer, variables, functions, i).map(|(i,(n, args))| (i, Value::Call(n, args)))
    )
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

        tag("&&"),
        tag("||"),

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

        "&&" => Operator::And(val),
        "||" => Operator::Or(val),

        "&" => Operator::BAnd(val),
        "|" => Operator::BOr(val),
        "<<" => Operator::Left(val),
        ">>" => Operator::Right(val),

        _ => {println!("{}", op) ;unreachable!()},
    }
}

fn exp_while<'a>(variables: &mut HashMap<&'a str, DataType>, functions: & HashMap<&'a str, (DataType, Vec<ParameterName<'a>>, FuncBody<'a>)>, i: &'a str) -> IResult<&'a str, Operator<'a>> {
    Ok(tuple_mut!(
        i,
        tag("?*"),
        (|i| expression(variables, functions, i)),
        tag(":"),
        (|i| expression(variables, functions, i)),
        binary_operator
    ).map(|(i, (_, cond, _, body,  op))| (i, Operator::While(Box::new(cond),Box::new(map_binary_operator(op, Value::Bracket(Box::new(body))))))).unwrap())
}

fn exp_set<'a>(variables: &mut HashMap<&'a str, DataType>, i: &'a str, i_: &'a str) -> IResult<&'a str, Operator<'a>> {
    Ok(tuple_mut!(
        i,
        tag("$"),
        variable_name
    ).map(|(i, (_, name))| {
            if let Some(data_type) = variables.get(name) {
                if !matches!(data_type, DataType::Integer) && !matches!(data_type, DataType::Any) {
                    return Err(nom::Err::Error(nom::error::Error::new(i_, ErrorKind::Verify)))
                }
            } else {
                variables.insert(name, DataType::Integer);
            }
            Ok((i, Operator::Set(name)))
    }).unwrap()?)
}

fn exp_if<'a>(variables: &mut HashMap<&'a str, DataType>, functions: & HashMap<&'a str, (DataType, Vec<ParameterName<'a>>, FuncBody<'a>)>, i: &'a str) -> IResult<&'a str, Operator<'a>> {
    Ok(tuple_mut!(
        i,
        tag("?"),
        (|i| expression(variables, functions, i)),
        tag(":"),
        (|i| expression(variables, functions, i))
    ).map(|(i, (_, t, _, f))| (i, Operator::If(Box::new(t), Box::new(f)))).unwrap())
}

fn exp_op<'a>(variables: &mut HashMap<&'a str, DataType>, functions: & HashMap<&'a str, (DataType, Vec<ParameterName<'a>>, FuncBody<'a>)>, i_: &'a str) -> IResult<&'a str, Operator<'a>> {
    let i = i_;
    Ok(tuple_mut!(
        i,
        whitespace0,
        binary_operator,
        whitespace0,
        (|i| value(variables, functions, i)),
        whitespace0
    ).map(|(i, (_ , op, _, val, _))| (i, (op, val)))
    .map(|(i, (op, val))| (i, map_binary_operator(op, val))).unwrap())
}

fn expression<'a>(variables: &mut HashMap<&'a str, DataType>, functions: & HashMap<&'a str, (DataType, Vec<ParameterName<'a>>, FuncBody<'a>)>, i_: &'a str) -> IResult<&'a str, Expression<'a>> {
    let (i, value) = preceded(
        whitespace0,
        |i| value(variables, functions, i)
    )(i_)?;
    tuple((
        many0(|i| {
            alt_mut!(
                exp_set(variables, i, i_),
                exp_while(variables, functions, i),
                exp_if(variables, functions, i),
                exp_op(variables, functions, i)
            )
        }),
        whitespace0
    ))(i).map(|(i, (ops, _))| (i, Expression{value, ops}))
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

fn literal<'a>(functions: &HashMap<&'a str, (DataType, Vec<ParameterName<'a>>, FuncBody<'a>)>,variables: &mut HashMap<&'a str, DataType>, i:&'a str) -> IResult<&'a str, Vec<Key<'a>>> {
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

fn characters<'a>(functions: &HashMap<&'a str, (DataType, Vec<ParameterName<'a>>, FuncBody<'a>)>, variables: &mut HashMap<&'a str, DataType>, i:&'a str) -> IResult<&'a str, Command<'a>> {
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

fn key<'a>(functions: &HashMap<&'a str, (DataType, Vec<ParameterName<'a>>, FuncBody<'a>)>, variables: &mut HashMap<&'a str, DataType>, i:&'a str) -> IResult<&'a str, Command<'a>> {
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

fn hold<'a>(functions: &HashMap<&'a str, (DataType, Vec<ParameterName<'a>>, FuncBody<'a>)>, variables: &mut HashMap<&'a str, DataType>, i:&'a str) -> IResult<&'a str, Command<'a>> {
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

fn if_<'a>(variables: &mut HashMap<&'a str, DataType>, functions: & HashMap<&'a str, (DataType, Vec<ParameterName<'a>>, FuncBody<'a>)>, nest_stack: &mut Vec<NestType>, i:&'a str) -> IResult<&'a str, Command<'a>> {
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

fn if_else<'a>(variables: &mut HashMap<&'a str, DataType>, functions: & HashMap<&'a str, (DataType, Vec<ParameterName<'a>>, FuncBody<'a>)>, nest_stack: &Vec<NestType>, i: &'a str) -> IResult<&'a str, Command<'a>> {
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

fn while_<'a>(variables: &mut HashMap<&'a str, DataType>, functions: & HashMap<&'a str, (DataType, Vec<ParameterName<'a>>, FuncBody<'a>)>, nest_stack: &mut Vec<NestType>, i:&'a str) -> IResult<&'a str, Command<'a>> {
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
        if !matches!(last, NestType::Function(..)) && !matches!(last, NestType::Pipe){
            return tag(">").map(|_| Command::End).parse(i)
            .and_then(|res| {
                nest_stack.pop();
                Ok(res)
            })
        }
    }
    
    Err(nom::Err::Error(Error::from_error_kind(i, ErrorKind::Char)))
    
}

fn set<'a>(variables: &mut HashMap<&'a str, DataType>, functions: & HashMap<&'a str, (DataType, Vec<ParameterName<'a>>, FuncBody<'a>)>, i_:&'a str) -> IResult<&'a str, Command<'a>> {
    let (i, (name, data)) = delimited(
        tag("<="), 
        separated_pair(
            variable_name, 
            tag(";"),
            |i| alt_mut!(
                delimited(tag("'"), |i|expression(variables, functions, i), tag("'"))
                    .map(|exp| Parameter::Expression(exp)).parse(i),
                literal(functions, variables, i).map(|(i, lit)| (i, Parameter::Literal(lit)))
            )
        ),
        tag(">")
    )(i_)?;

    if let Some(dtype) = variables.get(name) {
        match dtype {
            DataType::Integer => match data {
                Parameter::Expression(_) => (),
                Parameter::Literal(_) => return Err(nom::Err::Error(Error::from_error_kind(i_, ErrorKind::Verify))),
            },
            DataType::Literal => match data {
                Parameter::Expression(_) => return Err(nom::Err::Error(Error::from_error_kind(i_, ErrorKind::Verify))),
                Parameter::Literal(_) => (),
            },
            DataType::Any => (),
        }
    } else {
        match data {
            Parameter::Expression(_) => variables.insert(name, DataType::Integer),
            Parameter::Literal(_) => variables.insert(name, DataType::Literal),
        };
    }

    Ok((i, Command::Set(name, data)))
    
}

fn exp<'a>(variables: &mut HashMap<&'a str, DataType>, functions: & HashMap<&'a str, (DataType, Vec<ParameterName<'a>>, FuncBody<'a>)>, i:&'a str) -> IResult<&'a str, Command<'a>> { 
    delimited(
        tag("'"),
        |i| expression(variables, functions, i),
        tag("'")
    ).map(|e| Command::Expression(e))
    .parse(i)
}

fn ascii_escape<'a>(variables: &mut HashMap<&'a str, DataType>, functions: & HashMap<&'a str, (DataType, Vec<ParameterName<'a>>, FuncBody<'a>)>, i:&'a str) -> IResult<&'a str, Key<'a>> { 
    delimited(
        tag("\\@"),
        |i| expression(variables, functions, i),
        tag("\\")
    ).map(|e| Key::ASCII(e))
    .parse(i)
}

fn move_<'a>(variables: &mut HashMap<&'a str, DataType>, functions: & HashMap<&'a str, (DataType, Vec<ParameterName<'a>>, FuncBody<'a>)>, i:&'a str) -> IResult<&'a str, Command<'a>> {
    delimited(
        tag("<%"), 
        |i| {
            Ok(tuple_mut!(
                i,
                (|i| expression(variables, functions, i)), 
                tag(";"),
                (|i| expression(variables, functions, i))
            ).map(|(i, (a,_,b))| (i, (a,b))).unwrap())
        },
        tag(">")
    )
    .map(|(x, y)| Command::Move(x, y)).parse(i)
}

fn pipe<'a>(nest_stack: &mut Vec<NestType>,i:&'a str) -> IResult<&'a str, Command<'a>> {
    let (i, _) = tag("<|")(i)?;
    
    nest_stack.push(NestType::Pipe);
    
    let res =  terminated(inside_pipe, tag(">"))(i);

    let (tail, coms) = match res {
        Ok(res) => res,
        Err(e) => {
            nest_stack.pop();
            return Err(e)
        }
    };

    nest_stack.pop();
    Ok((tail, Command::Pipe(coms)))
}

fn call_components<'a>(expected: DataType, variables: &mut HashMap<&'a str, DataType>, functions: & HashMap<&'a str, (DataType, Vec<ParameterName<'a>>, FuncBody<'a>)>, i_:&'a str) -> IResult<&'a str, (&'a str, Vec<Parameter<'a>>)> {
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

fn call<'a>(variables: &mut HashMap<&'a str, DataType>, functions: & HashMap<&'a str, (DataType, Vec<ParameterName<'a>>, FuncBody<'a>)>, i: &'a str) -> IResult<&'a str, Command<'a>> {
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

fn hex<'a>(i:&'a str) -> IResult<&'a str, u8> {
    pair(
        hex_digit,
        hex_digit
    ).map(|(a, b)| a as u8 * 16 + b as u8).parse(i)
}   
fn escape_inner<'a>(i:&'a str) -> IResult<&'a str, Key<'a>> {
    alt((
        preceded(tag("\\x"), hex).map(|hex| Key::Keycode(hex)),
        tag("\\n").map(|_| Key::Special(SpecialKey::Enter)),
        tag("\\t").map(|_| Key::Special(SpecialKey::Tab)),
        tag("\\b").map(|_| Key::Special(SpecialKey::Backspace)),
        tag("\\l").map(|_| Key::Left),
        tag("\\r").map(|_| Key::Right),
        tag("\\m").map(|_| Key::Middle),
        tag("\\\"").map(|_| Key::Literal('\"')),
        tag("\\\'").map(|_| Key::Literal('\'')),
        tag("\\\\").map(|_| Key::Literal('\\')),
        tag("\\<").map(|_| Key::Literal('<')),
    ))(i)
}

fn sleep<'a>(functions: &HashMap<&'a str, (DataType, Vec<ParameterName<'a>>, FuncBody<'a>)>, variables: &mut HashMap<&'a str, DataType>, i:&'a str) -> IResult<&'a str, Command<'a>> {
    delimited(
        tag("<*'"), 
        |i| expression(variables, functions, i), 
        tag("'>")
    ).map(|exp| Command::Sleep(exp)).parse(i)
}

fn escape<'a>(functions: &HashMap<&'a str, (DataType, Vec<ParameterName<'a>>, FuncBody<'a>)>, variables: &mut HashMap<&'a str, DataType>, i:&'a str) -> IResult<&'a str, Vec<Key<'a>>> {
    many1(|i|{
        alt_mut!(
            variable_escape(variables, i),
            ascii_escape(variables, functions, i),
            escape_inner(i)
        )
    })(i)
}

fn inside_pipe<'a>(i:&'a str) -> IResult<&'a str, &'a str> {
    many1(
        alt((
            tag("\\<").map(|_| ()),
            tag("\\>").map(|_| ()),
            tag("\\\\").map(|_| ()),
            take_while1(|c:char| c != '<' && c != '>' && c != '\\').map(|_| ()),
        ))
    )(i).map(|(tail, _)| {
        let (_, coms) = i.take_split(i.len()- tail.len());
        (tail, coms)
    })
}

fn exit<'a>(i:&'a str) -> IResult<&'a str, Command<'a>> {
    preceded(whitespace0, eof).map(|_| Command::Exit).parse(i)
}


fn command<'a>(variables: &mut HashMap<&'a str, DataType>, functions: &mut  HashMap<&'a str, (DataType, Vec<ParameterName<'a>>, FuncBody<'a>)>, nest_stack: &mut Vec<NestType>, i:&'a str) -> IResult<&'a str, Command<'a>> {
    delimited(
        take_while(|c: char| c != ' ' && c.is_whitespace()),
        |i| alt_mut!(
            literal(functions, variables, i).map(|(i, l)| (i, Command::Literal(l))),
            escape(functions, variables, i).map(|(i, k)| (i, Command::Key(k))),
            led.map(|l| Command::LED(l)).parse(i),
            if_else(variables, functions, nest_stack, i),
            else_(nest_stack, i),
            key(functions, variables, i),
            hold(functions, variables, i),
            sleep(functions, variables, i),
            exp(variables, functions, i),
            move_(variables, functions, i),
            call(variables, functions, i),
            if_(variables, functions, nest_stack, i),
            while_(variables, functions, nest_stack, i),
            pipe(nest_stack, i),
            end(nest_stack, i),
            set(variables, functions, i),
            exit(i),
            function(variables, functions, nest_stack, i)
        ),
        take_while(|c: char| c != ' ' && c.is_whitespace())
    )(i)
}

fn function<'a>(variables: &mut HashMap<&'a str, DataType>, functions: & mut  HashMap<&'a str, (DataType, Vec<ParameterName<'a>>, FuncBody<'a>)>, nest_stack: &mut Vec<NestType>, i_:&'a str) -> IResult<&'a str, Command<'a>> {
    if nest_stack.len() > 0 {
        return Err(nom::Err::Error(nom::error::Error::from_error_kind(i_, ErrorKind::Verify)))
    }

    let (i, (fn_type, name, params)) = preceded(
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
    )(i_)?;

    nest_stack.push(NestType::Function(None));

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

    functions.insert(name, (fn_type.clone(), params, FuncBody::Literal("")));

    let res = terminated(
            |i| match fn_type {
                DataType::Literal => many1(delimited(whitespace0, |i| command(variables, functions, nest_stack, i), whitespace0))(i),
                DataType::Integer => delimited(
                    tag("'"),
                    |i| expression(variables, functions, i),
                    tag("'")
                )(i).map(|(i, e)| (i, vec![Command::Expression(e)])),
                _ => unreachable!()
            },
        preceded(whitespace0, tag(">"))
    )(i);

    let (i, coms) = if let Ok((tail, mut coms)) = res  {
        match fn_type {
            DataType::Integer => (tail, FuncBody::Expression(match coms.pop() {
                Some(Command::Expression(exp)) => exp,
                _ => unreachable!()
            })),
            DataType::Literal => {
                let (_, coms) = i.take_split(i.len()- tail.len()-1);
                (tail, FuncBody::Literal(coms))
            },
            DataType::Any => unreachable!(),
        }
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
    .map(|_| ()).parse(i)
}

pub enum NestType<'a> {
    While,
    If,
    Pipe,
    Function(Option<(&'a str, Vec<(&'a str, DataType)>)>),
}

#[derive(Clone)]
pub enum FuncBody<'a> {
    Expression(Expression<'a>),
    Literal(&'a str)

}

pub struct BorkParser<'a> {
    nest_stack: Vec<NestType<'a>>,
    functions:  HashMap<&'a str, (DataType, Vec<ParameterName<'a>>, FuncBody<'a>)>,
    variables: HashMap<&'a str, DataType>,
}


impl<'a> BorkParser<'a> {
    pub fn new() -> BorkParser<'a> {
        BorkParser { nest_stack: Vec::new(), functions: HashMap::new(), variables: HashMap::new() }
    }

    pub fn remove_func(&mut self, name: &'a str) -> Option<(DataType, Vec<ParameterName<'a>>, FuncBody<'a>)> {
        self.functions.remove(name)
    }

    pub fn add_func(&mut self, name: &'a str, fn_type: DataType, params: Vec<ParameterName<'a>>, body: FuncBody<'a>) {
        self.functions.insert(name, (fn_type, params, body));
    }

    pub fn begin_function(&mut self, name: &'a str) {
        let mut saved = vec![];
        if let Some((_, params, _)) = self.functions.get(name) {
            for param in params {
                match param {
                    ParameterName::Expression(name) => if let Some(var) = self.variables.insert(&name, DataType::Integer) {
                        saved.push((*name, var));
                    },
                    ParameterName::Literal(name) => if let Some(var) = self.variables.insert(&name, DataType::Literal) {
                        saved.push((*name, var));
                    },
                }
            }
        }
        self.nest_stack.push(NestType::Function(Some((name, saved))))
    }

    pub fn end_function(&mut self) {
        if let Some(NestType::Function(..)) =  self.nest_stack.last() {
            if let Some(NestType::Function(Some((name, saved)))) =  self.nest_stack.pop() {
                if let Some((_, params, _)) = self.functions.get(name) {
                    for param in params {
                        match param {
                            ParameterName::Expression(name) => self.variables.remove(name),
                            ParameterName::Literal(name) => self.variables.remove(name),
                        };
                    }
                }
                self.variables.extend(saved.into_iter())
            }
        }
    }

    pub fn get_level(&self) -> usize {
        self.nest_stack.len()
    }

    pub fn get_level_type(&self) -> Option<&NestType> {
        self.nest_stack.last()
    }

    pub fn jmp_end(&mut self, i_:&'a str) -> IResult<&'a str, &'a str> {
        if self.nest_stack.len() == 0{
            return Ok((i_, i_))
        }

        let level = self.nest_stack.len();

        let mut i = i_;
        while self.nest_stack.len() >= level {
            let (new_i, com) = self.parse_command(i)?;
            i = new_i;
            if level > self.nest_stack.len() {
                match com {
                    Command::End => break,
                    Command::Exit => break,
                    _ => (),
                }
            }
        }

        Ok((i, i_))
    }

    pub fn jmp_next(&mut self, i_:&'a str) -> IResult<&'a str, &'a str> {
        if self.nest_stack.len() == 0{
            return Ok((i_, i_))
        }

        let level = self.nest_stack.len();

        let mut i = i_;
        while self.nest_stack.len() >= level {
            let (new_i, com) = self.parse_command(i)?;
            if level >= self.nest_stack.len() {
                match com {
                    Command::If(_) => break,
                    Command::ElseIf(_) => break,
                    Command::Else => break,
                    Command::While(_) => break,
                    Command::End => break,
                    Command::Exit => break,
                   _ => (),
                }
            }
            i = new_i;
        }

        Ok((i, i_))
    }


    pub fn parse_command(&mut self, i:&'a str) -> IResult<&'a str, Command<'a>> {
        let (i, _) = if self.nest_stack.len() > 0 {
            whitespace0(i)
        } else {
            take_while(|c| c == '\n').map(|_| ()).parse(i)
        }.unwrap_or((i, ()));

        let (i, com) = command(&mut self.variables, &mut self.functions, &mut self.nest_stack, i).or_else(|e|
            if self.nest_stack.len() != 0 {
                Err(e)
            } else {
                characters(&self.functions, &mut self.variables, i)
            }
        )?;

        let (i, _) = if self.nest_stack.len() > 0 {
            whitespace0(i)
        } else {
            take_while(|c| c == '\n').map(|_| ()).parse(i)
        }.unwrap_or((i, ()));

        Ok((i, com))
    }
}


#[cfg(test)]
mod tests {
    use virt_hid::key::{SpecialKey, Modifier, LEDState};
    use crate::parser::{Command, Key, Value, Operator, Parameter, function};

    use super::BorkParser;


    #[test]
    fn test() {
        let mut borker = BorkParser::new();

        assert!(matches!(borker.parse_command("     \t\n     ").unwrap().1, Command::Exit));

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

        let (_, com) = borker.parse_command("<*'1000'>").unwrap();
        if let Command::Sleep(exp) = com {
            assert_eq!(exp.ops.len(), 0);
            assert!(matches!(exp.value, Value::Int(1000)));
        } else {
            assert!(false);
        }

        let (_, com) = borker.parse_command(">").unwrap();
        assert!(matches!(com, Command::Literal(_a)));

        if let Command::Set(name, Parameter::Expression(exp)) = borker.parse_command("<=x;'10'>").unwrap().1 {
            assert_eq!(name, "x");
            assert!(matches!(exp.value, Value::Int(10)));
        } else {
            assert!(false);
        }

        if let Command::Expression(exp) = borker.parse_command("'10$x'>").unwrap().1 {
            assert!(matches!(exp.value, Value::Int(10)));
            assert_eq!(exp.ops.len(), 1);
            assert!(matches!(exp.ops[0], Operator::Set("x")));
        } else {
            assert!(false);
        }

        if let Command::Key(s) = borker.parse_command("\\$x\\").unwrap().1 {
            assert_eq!(s.len(), 1);
            assert!(matches!(s[0], Key::Variable("x")));
        } else {
            assert!(false);
        }

        if let Command::Expression(exp) = borker.parse_command("'x*10'").unwrap().1 {
            assert!(matches!(exp.value, Value::Variable("x")));
            assert_eq!(exp.ops.len(), 1);
            assert!(matches!(exp.ops[0], Operator::Mult(Value::Int(10))))
        } else {
            assert!(false);
        }

        if let Command::Expression(mut exp) = borker.parse_command("'10*10 ?* x : 10 +'").unwrap().1 {
            assert_eq!(exp.ops.len(), 2);
            assert!(matches!(exp.value, Value::Int(10)));
            assert!(matches!(exp.ops[0], Operator::Mult(Value::Int(10))));
            if let Operator::While(cond, op) = exp.ops.remove(1) {
                assert!(matches!(cond.value, Value::Variable("x")));
                assert_eq!(cond.ops.len(), 0);
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

        if let Command::Expression(mut exp) = borker.parse_command("'x?1:0'").unwrap().1 {
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

        if let Command::Expression(exp) = borker.parse_command("'10$x'").unwrap().1 {
            assert!(matches!(exp.value, Value::Int(10)));
            assert_eq!(exp.ops.len(), 1);
            assert!(matches!(exp.ops[0], Operator::Set("x")))
        } else {
            assert!(false);
        }

        if let Command::Expression(exp) = borker.parse_command("'\\&1'").unwrap().1 {
            assert!(matches!(exp.value, Value::LED(LEDState::NumLock)));
            assert_eq!(exp.ops.len(), 0);
        } else {
            assert!(false);
        }

        assert!(matches!(borker.parse_command("\\&1").unwrap().1, Command::LED(LEDState::NumLock)));

        if let Command::Expression(exp) = borker.parse_command("'@a'").unwrap().1 {
            assert!(matches!(exp.value, Value::Int(97)));
            assert_eq!(exp.ops.len(), 0);
        } else {
            assert!(false);
        }

        if let Command::Move(x, y) = borker.parse_command("<%10;-20>").unwrap().1 {
            assert!(matches!(x.value, Value::Int(10)));
            assert!(matches!(y.value, Value::Int(-20)));
        } else {
            assert!(false);
        }

        let (_, coms) = borker.parse_command("<|ls /dev/loop\\<10>").unwrap();
        if let Command::Pipe(coms) = coms {
            assert_eq!(coms, "ls /dev/loop\\<10");
        } else {
            assert!(false);
        }


        assert!(matches!(function(&mut borker.variables, &mut borker.functions, &mut borker.nest_stack, "<+\"hello;\"name;\"Hello my name is\\$x\\\">").unwrap(), ("", Command::None)));
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

        assert!(matches!(function(&mut borker.variables, &mut borker.functions, &mut borker.nest_stack, "<+#add;#a;#b;'a+b'>").unwrap(), ("", Command::None)));
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
    fn jump() {
        let mut borker = BorkParser::new();
        let (i, _) = borker.parse_command(r#"
'10$x'
<?x>0;
    "x greater than 0"
;?x<0;
    "x less than 0"
;
    "x equal to 0"
>"#).unwrap();

        let (i, _) = borker.parse_command(i).unwrap();
        let (i, _) = borker.jmp_next(i).unwrap();
        assert_eq!(i, r#";?x<0;
    "x less than 0"
;
    "x equal to 0"
>"#);
        let (i, _) = borker.parse_command(i).unwrap();
        let (i, _) = borker.jmp_next(i).unwrap();
        assert_eq!(i, r#";
    "x equal to 0"
>"#);
        let (i, _) = borker.parse_command(i).unwrap();
        let (i, _) = borker.jmp_next(i).unwrap();
        assert_eq!(i, r#">"#);
    }

    #[test]
    fn jump_end() {
        let mut borker = BorkParser::new();
        let (i, _) = borker.parse_command(r#"
'0$x'
<?x>0;
    "x greater than 0"
;?x<0;
    "x less than 0"
;
    "x equal to 0"
>"#).unwrap();

        let (i, _) = borker.parse_command(i).unwrap();
        let (i, _) = borker.jmp_end(i).unwrap();
        assert_eq!(i, r#""#);
    }

    #[test]
    fn fib() {
        let mut borker = BorkParser::new();
        let (mut i, mut com) = borker.parse_command(r#"
<+#fib;#x;'
    x <= 1 ? 
        x < 1 ? 0 : 1
    :
        <!fib;x-1> + <!fib;x-2>
'>

The fibonacci of 10 is <!fib;10>."#).unwrap();
    
        while !matches!(com, Command::Exit) {
            (i, com) = borker.parse_command(i).unwrap();
        }
    }

    #[test]
    fn greeting() {
        let mut borker = BorkParser::new();
        let (mut i, mut com) = borker.parse_command(r#"
Hello <|echo $USER>,\n
\n
How are you doing today?
"#).unwrap();
    
        while !matches!(com, Command::Exit) {
            (i, com) = borker.parse_command(i).unwrap();
        }
    }
}
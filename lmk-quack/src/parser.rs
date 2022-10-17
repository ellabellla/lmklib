use lmk_hid::key::{SpecialKey, Modifier, Key, KeyOrigin};
use nom::character::complete::{digit1, alpha1, space1, space0};
use nom::combinator::{eof};
use nom::bytes::complete::{take, take_while, take_till};
use nom::error::Error;
use nom::multi::{many1, many0};
use nom::sequence::tuple;
use nom::{IResult, bytes::complete::tag, Parser};
use nom::branch::alt;

#[derive(Debug)]
pub enum Operator<'a>{
    Add(Value<'a>),
    Sub(Value<'a>),
    Mult(Value<'a>),
    Div(Value<'a>),
    Mod(Value<'a>),
    Exp(Value<'a>),

    Equ(Value<'a>),
    Not(Value<'a>),
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
}

#[derive(Debug)]
pub struct Expression<'a> {
    pub value: Value<'a>,
    pub ops: Vec<Operator<'a>>
}

#[derive(Debug)]
pub enum Command<'a> {
    Rem(&'a str),
    String(&'a str),
    StringLN(&'a str),
    Special(SpecialKey),
    Modifier(Modifier),
    Shortcut(Vec<Modifier>, Key),
    Delay(Expression<'a>),
    Hold(Key),
    Release(Key),
    HoldMod(Modifier),
    ReleaseMod(Modifier),
    InjectMod,
    Var(&'a str, Expression<'a>),
    If(Value<'a>),
    ElseIf(Value<'a>),
    Else,
    EndIf,
    While(Value<'a>),
    EndWhile,
    Function(&'a str),
    EndFunction,
    Call(&'a str),
    None,
}

#[derive(Debug)]
pub enum Value<'a> {
    Int(i64),
    Variable(&'a str),
    Bracket(Box<Expression<'a>>),
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
    let (_, int) = alt((
        tag("TRUE").map(|_| 1),
        tag("FALSE").map(|_| 0),
    ))(i)?;

    Ok((i, Value::Int(int)))
}

fn variable<'a>(i: &'a str) -> IResult<&'a str, &'a str> {
    let (i, _) = tag("$")(i)?;
    let (i, name) = alpha1(i)?;
    Ok((i, name))
}

pub fn bracket<'a>(i: &'a str) -> IResult<&'a str, Value> {
    tuple((
        tag("("),
        space0,
        expression,
        space0,
        tag(")")
    ))(i).map(|(i, (_,_,expr, _,_))| (i, Value::Bracket(Box::new(expr))))
}

fn value<'a>(i: &'a str) -> IResult<&'a str, Value<'a>> {
    alt((
        integer,
        variable.map(|name| Value::Variable(name)),
        bool,
        bracket
    ))(i)
}

fn delay<'a>(i: &'a str) -> IResult<&'a str, Command<'a>> {
     tuple((
        tag("DELAY"),
        space1,
        expression
    ))(i)
        .map(|(i, (_,_,value))| (i, Command::Delay(value)))
}

fn key<'a>(i: &'a str) -> IResult<&'a str, Key> {
    alt((
        special
            .map(|s| Key::Special(s)),
        take(1u32)
            .map(|c:&str| Key::Char(c.chars().next().unwrap(), KeyOrigin::Keyboard))
    ))(i)
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

fn modifiers<'a>(i: &'a str) -> IResult<&'a str, Vec<Modifier>> {
    many1(tuple((
        modifier,
        space1
    )))(i).map(|(i, mods)| (i, mods.iter().map(|(modi, _)| *modi).collect::<Vec<Modifier>>()))
}

fn modifier_command<'a>(i: &'a str) -> IResult<&'a str, Command<'a>> {
    modifier(i).map(|(i, modifier)| (i, Command::Modifier(modifier)))
}

fn shortcut<'a>(i: &'a str) -> IResult<&'a str, Command<'a>> {
    let (i, (modifiers, key)) = tuple((
        modifiers,
        key
    ))(i)?;
    Ok((i, Command::Shortcut(modifiers, key)))
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
        tag("PAUSE BREAK").map(|_| SpecialKey::Pause),
        tag("PRINT SCREEN").map(|_| SpecialKey::PrintScreen),
        tag("MENU APP").map(|_| SpecialKey::Menu),
    ))(i)
}

fn lock<'a>(i: &'a str) -> IResult<&'a str, Command<'a>>  {
    alt((
        tag("CAPSLOCK").map(|_| Command::Special(SpecialKey::CapsLock)),
        tag("NUMLOCK").map(|_| Command::Special(SpecialKey::NumLockAndClear)),
        tag("SCROLLOCK").map(|_| Command::Special(SpecialKey::ScrollLock)),
    ))(i)
}


fn special_command<'a>(i: &'a str) -> IResult<&'a str, Command<'a>>  {
    special(i).map(|(i, s)| (i, Command::Special(s)))
}

pub fn take_till_no_end<F, Input, Error: nom::error::ParseError<Input>>(
    cond: F,
) -> impl Fn(Input) -> IResult<Input, Input, Error>
where
    Input: nom::InputTakeAtPosition + nom::InputLength + nom::Slice<std::ops::RangeFrom<usize>>,
    F: Fn(<Input as nom::InputTakeAtPosition>::Item) -> bool,
{
    move |i: Input| {
        match i.split_at_position::<_, Error>(|c| cond(c)) {
            Ok(res) => Ok(res),
            Err(e) => match e {
                nom::Err::Incomplete(_) => Ok((i.slice(i.input_len()..), i)),
                nom::Err::Error(_) => Err(e),
                nom::Err::Failure(_) => Err(e),
            },
        }
    }
}


pub fn hold_release<'a>(i: &'a str) -> IResult<&'a str, Command<'a>> {
    alt((
        tuple((
            tag("HOLD"),
            space1,
            modifier
        ))
            .map(|(_, _, key)| Command::HoldMod(key)),
        tuple((
            tag("RELEASE"),
            space1,
            modifier
        ))
            .map(|(_, _, key)| Command::ReleaseMod(key)),
        tuple((
            tag("HOLD"),
            space1,
            key
        ))
            .map(|(_, _, key)| Command::Hold(key)),
        tuple((
            tag("RELEASE"),
            space1,
            key
        ))
            .map(|(_, _, key)| Command::Release(key)),
    ))(i)
}

pub fn string<'a>(i: &'a str) -> IResult<&'a str, Command<'a>> {
    alt((
        tuple((
            tag("STRING"),
            space1,
            take_till_no_end(|c| c == '\n'),
        )).map(|(_, _, str)| Command::String(str)),
        tuple((
            tag("STRINGLN"),
            space1,
            take_till_no_end(|c| c == '\n'),
        )).map(|(_, _, str)| Command::StringLN(str)),
    ))(i)
}

pub fn expression<'a>(i: &'a str) -> IResult<&'a str, Expression> {
    tuple((
        value,
        many0(
            tuple((
                space0,
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

                )),
                space0,
                value,
            )).map(|(_ , op, _, val)| match op {
                "+" => Operator::Add(val),
                "-" => Operator::Sub(val),
                "*" => Operator::Mult(val),
                "/" => Operator::Div(val),
                "%" => Operator::Mod(val),
                "^" => Operator::Exp(val),

                "==" => Operator::Equ(val),
                "!=" => Operator::Not(val),
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

                _ => unreachable!(),
            })
        )
    ))(i).map(|(i, (value, ops))| (i, Expression{value, ops}))
}

fn var<'a>(i: &'a str) -> IResult<&'a str, Command<'a>> {
    tuple((
        tag("VAR"),
        space1,
        variable,
        space0,
        tag("="),
        space0,
        expression,
    ))(i).map(|(i, (_,_, var, _,_,_,expr))| (i, Command::Var(var, expr)))
}

fn if_begin<'a>(i: &'a str) -> IResult<&'a str, Command<'a>> {
    tuple((
        tag("IF"),
        space0,
        alt((
            bool,
            bracket,
        )),
        space0,
        tag("THEN")
    ))(i).map(|(i,(_,_,val,_,_))| (i, Command::If(val)))
}

fn if_else<'a>(i: &'a str) -> IResult<&'a str, Command<'a>> {
    tuple((
        tag("ELSE IF"),
        space0,
        alt((
            bool,
            bracket,
        )),
        space0,
        tag("THEN")
    ))(i).map(|(i,(_,_,val,_,_))| (i, Command::ElseIf(val)))
}

fn else_control<'a>(i: &'a str) -> IResult<&'a str, Command<'a>> {
    tag("ELSE")(i).map(|(i,_)| (i, Command::Else))
}

fn if_end<'a>(i: &'a str) -> IResult<&'a str, Command<'a>> {
    tag("END_IF")(i).map(|(i,_)| (i, Command::EndIf))
}

fn if_control<'a>(i: &'a str) -> IResult<&'a str, Command<'a>> {
    alt((
        if_begin,
        if_else,
        else_control,
        if_end,
    ))(i)
}

fn while_begin<'a>(i: &'a str) -> IResult<&'a str, Command<'a>> {
    tuple((
        tag("WHILE"),
        space0,
        alt((
            bool,
            bracket,
        )),
        space0,
    ))(i).map(|(i,(_,_,val,_))| (i, Command::While(val)))
}

fn while_end<'a>(i: &'a str) -> IResult<&'a str, Command<'a>> {
    tag("END_WHILE")(i).map(|(i,_)| (i, Command::EndWhile))
}

fn while_control<'a>(i: &'a str) -> IResult<&'a str, Command<'a>> {
    alt((
        while_begin,
        while_end,
    ))(i)
}

fn function_begin<'a>(i: &'a str) -> IResult<&'a str, Command<'a>> {
    tuple((
        tag("FUNCTION"),
        space1,
        take_till(|c| c == '('),
        tag("()"),
    ))(i).map(|(i,(_,_,name,_))| (i, Command::Function(name)))
}

fn function_end<'a>(i: &'a str) -> IResult<&'a str, Command<'a>> {
    tag("END_FUNCTION")(i).map(|(i,_)| (i, Command::EndFunction))
}

fn call<'a>(i: &'a str) -> IResult<&'a str, Command<'a>> {
    tuple((
        take_till(|c| c == '('),
        tag("()"),
    ))(i).map(|(i,(name,_))| (i, Command::Call(name)))
}

fn function_control<'a>(i: &'a str) -> IResult<&'a str, Command<'a>> {
    alt((
        function_begin,
        function_end,
        call,
    ))(i)
}

pub fn parse_function<'a>(i: &'a str) -> IResult<&'a str, &'a str> {
    tuple((
        space0,
        tag("FUNCTION"),
        space1,
        take_till(|c| c == '('),
        tag("()"),
        space0,
        take_while(|c| c == '\n'),
        eof
    ))(i)
        .map(|(i, (_, _, _, name, _, _, _, _))| (i, name))
}

pub fn parse_line<'a>(i: &'a str) -> IResult<&'a str, Command<'a>> {
    alt((
        tuple((
            space0,
            alt((
                tuple((
                    tag("REM"),
                    space1,
                    take_till_no_end(|c| c == '\n'),
                )).map(|(_, _, str)| Command::Rem(str)),
                string,
                tag("INJECT_MOD").map(|_| Command::InjectMod),
                delay,
                shortcut,
                special_command,
                hold_release,
                modifier_command,
                lock,
                var,
                if_control,
                while_control,
                function_control,
            )),
            space0,
            take_while(|c| c == '\n'),
            eof
        ))
            .map(|(_, command, _, _, _)| command),
        tuple((
            space0,
            take_while(|c| c == '\n'),
            eof
        ))
            .map(|_| Command::None)
    ))(i)
}


pub fn parse_define<'a>(i: &'a str) -> IResult<&'a str, (&'a str, &'a str)> {
    let (i, (_, _, _, name, _, text, _, _, _)) = tuple((
        space0,
        tag("DEFINE"),
        space1,
        alpha1,
        space1,
        take_till_no_end(|c| c == '\n'),
        space0,
        take_while(|c| c == '\n'),
        eof
    ))(i)?; 

    Ok((i, (name, text)))
}

#[cfg(test)]
mod tests {
    use lmk_hid::key::{Key, KeyOrigin, Modifier, SpecialKey};

    use crate::{parser::{parse_line, Command, Value, parse_define, Operator}};

    #[test]
    pub fn test() {
        assert!(matches!(parse_define("DEFINE NAME stuff and things\n").unwrap().1, ("NAME", "stuff and things")));

        assert!(matches!(parse_line("REM a comment\n").unwrap().1, Command::Rem("a comment")));

        assert!(matches!(parse_line("STRING a string\n").unwrap().1, Command::String("a string")));
        assert!(matches!(parse_line("STRINGLN a string\n").unwrap().1, Command::StringLN("a string")));

        assert!(matches!(parse_line("INJECT_MOD\n").unwrap().1, Command::InjectMod));

        assert!(matches!(parse_line("DELAY 100\n").unwrap().1, Command::Delay(_expr)));

        assert!(matches!(parse_line("CTL SHIFT a\n").unwrap().1, Command::Shortcut(_mods, Key::Char('a', KeyOrigin::Keyboard))));

        assert!(matches!(parse_line("ENTER\n").unwrap().1, Command::Special(SpecialKey::Enter)));

        assert!(matches!(parse_line("HOLD a\n").unwrap().1, Command::Hold(Key::Char('a', KeyOrigin::Keyboard))));
        assert!(matches!(parse_line("RELEASE a\n").unwrap().1, Command::Release(Key::Char('a', KeyOrigin::Keyboard))));

        assert!(matches!(parse_line("HOLD GUI\n").unwrap().1, Command::HoldMod(Modifier::LeftMeta)));
        assert!(matches!(parse_line("RELEASE GUI\n").unwrap().1, Command::ReleaseMod(Modifier::LeftMeta)));

        assert!(matches!(parse_line("GUI\n").unwrap().1, Command::Modifier(Modifier::LeftMeta)));

        assert!(matches!(parse_line("CAPSLOCK\n").unwrap().1, Command::Special(SpecialKey::CapsLock)));

        let expr = parse_line("VAR $variable = 1 + (2 + 2)\n").unwrap().1;
        match expr {
            Command::Var(name, expr) => {
                assert_eq!(name, "variable");
                assert!(matches!(expr.value, Value::Int(1)));
                assert!(matches!(&expr.ops[0], Operator::Add(Value::Bracket(_a))));
                assert_eq!(expr.ops.len(), 1);
                if let Operator::Add(Value::Bracket(expr)) = &expr.ops[0] {
                    assert!(matches!(expr.value, Value::Int(2)));
                    assert!(matches!(&expr.ops[0], Operator::Add(Value::Int(2))));
                    assert_eq!(expr.ops.len(), 1);
                }
            },
            _ => assert!(false)
        }

        let if_begin = parse_line("IF (1 < 2) THEN\n").unwrap().1;
        assert!(matches!(&if_begin, Command::If(Value::Bracket(_a))));
        if let Command::If(Value::Bracket(expr)) = if_begin {
            assert!(matches!(expr.value, Value::Int(1)));
            assert!(matches!(expr.ops[0], Operator::Les(Value::Int(2))));
        }


        let if_else = parse_line("ELSE IF (1 < 2) THEN\n").unwrap().1;
        assert!(matches!(&if_else, Command::ElseIf(Value::Bracket(_a))));
        if let Command::ElseIf(Value::Bracket(expr)) = if_else {
            assert!(matches!(expr.value, Value::Int(1)));
            assert!(matches!(expr.ops[0], Operator::Les(Value::Int(2))));
        }

        assert!(matches!(parse_line("ELSE\n").unwrap().1, Command::Else));
        assert!(matches!(parse_line("END_IF\n").unwrap().1, Command::EndIf));

        let while_begin = parse_line("WHILE (1 < 2)\n").unwrap().1;
        assert!(matches!(&while_begin, Command::While(Value::Bracket(_a))));
        if let Command::While(Value::Bracket(expr)) = while_begin {
            assert!(matches!(expr.value, Value::Int(1)));
            assert!(matches!(expr.ops[0], Operator::Les(Value::Int(2))));
        }

        assert!(matches!(parse_line("END_WHILE\n").unwrap().1, Command::EndWhile));

        assert!(matches!(parse_line("FUNCTION hello()\n").unwrap().1, Command::Function("hello")));
        assert!(matches!(parse_line("END_FUNCTION\n").unwrap().1, Command::EndFunction));
        assert!(matches!(parse_line("hello()\n").unwrap().1, Command::Call("hello")));

        assert!(matches!(parse_line("").unwrap().1, Command::None));
        assert!(matches!(parse_line("    \t\n").unwrap().1, Command::None));
    }
}
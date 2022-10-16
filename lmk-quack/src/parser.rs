use lmk_hid::key::{SpecialKey, Modifier, Key, KeyOrigin};
use nom::character::complete::{digit1, alpha1, space1, space0};
use nom::combinator::{rest, eof};
use nom::bytes::complete::take;
use nom::multi::many1;
use nom::sequence::tuple;
use nom::{IResult, bytes::complete::tag, Parser};
use nom::branch::alt;


#[derive(Debug)]
pub enum Command<'a> {
    Rem,
    String(&'a str),
    StringLN(&'a str),
    Special(SpecialKey),
    Modifier(Modifier),
    Shortcut(Vec<Modifier>, Key),
    Delay(u64),
}

#[derive(Debug)]
pub enum Value<'a> {
    Int(u64),
    Constant(&'a str),
    Variable(&'a str),
}

fn integer<'a>(i: &'a str) -> IResult<&'a str, Value<'a>> {
    let (i, delay) = digit1(i)?;
    let int: u64 = delay.parse().unwrap();

    Ok((i, Value::Int(int)))
}

fn bool<'a>(i: &'a str) -> IResult<&'a str, Value<'a>> {
    let (_, int) = alt((
        tag("TRUE").map(|_| 1),
        tag("FALSE").map(|_| 0),
    ))(i)?;

    Ok((i, Value::Int(int)))
}

fn variable<'a>(i: &'a str) -> IResult<&'a str, Value<'a>> {
    let (i, _) = tag("$")(i)?;
    let (_, name) = alpha1(i)?;
    Ok((i, Value::Variable(name)))
}

fn value<'a>(i: &'a str) -> IResult<&'a str, Value<'a>> {
    alt((
        integer,
        variable,
        bool
    ))(i)
}

fn delay<'a>(i: &'a str) -> IResult<&'a str, Command<'a>> {
    let (i, _) = tag("DELAY")(i)?;
    let (i, delay) = digit1(i)?;
    let delay: u64 = delay.parse().unwrap();

    Ok((i, Command::Delay(delay)))
}

fn modifier<'a>(i: &'a str) -> IResult<&'a str, Vec<Modifier>> {
    many1(alt((
        tag("ALT").map(|_| Modifier::LeftAlt),
        tag("CTL").map(|_| Modifier::LeftControl),
        tag("CONTROL").map(|_| Modifier::LeftControl),
        tag("COMMAND").map(|_| Modifier::LeftMeta),
        tag("GUI").map(|_| Modifier::LeftMeta),
        tag("SHIFT").map(|_| Modifier::LeftShift),
    )))(i)
}

fn modifier_command<'a>(i: &'a str) -> IResult<&'a str, Vec<Command<'a>>> {
    modifier(i).map(|(i, modifiers)| (i, modifiers.iter().map(|m| Command::Modifier(*m)).collect()))
}

fn shortcut<'a>(i: &'a str) -> IResult<&'a str, Command<'a>> {
    let (i, (modifiers, _, key)) = tuple((
        modifier,
        space1,
        alt((
            special
                .map(|s| Key::Special(s)),
            take(1u32)
                .map(|c:&str| Key::Char(c.chars().next().unwrap(), KeyOrigin::Keyboard))
        ))
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


fn special_command<'a>(i: &'a str) -> IResult<&'a str, Command<'a>>  {
    special(i).map(|(i, s)| (i, Command::Special(s)))
}

pub fn parse_line<'a>(i: &'a str) -> IResult<&'a str, Command<'a>> {
    tuple((
        alt((
            alt((
                tag("REM").map(|_| Command::Rem), 
                tag("STRING")
                    .and_then(|i| space1(i))
                    .and_then(|i| Ok((i, Command::String(rest(i)?.1)))),
                tag("STRINGLN")
                    .and_then(|i| space1(i))
                    .and_then(|i| Ok((i, Command::StringLN(rest(i)?.1)))),
            )),
            shortcut,
            special_command,
            delay
        )),
        space0,
        eof
    ))(i)
        .map(|(i, (command, _, _))| (i, command))
}


pub fn parse_define<'a>(i: &'a str) -> IResult<&'a str, (&'a str, &'a str)> {
    let (i, (_, _, name, _, text, _, _)) = tuple((
        tag("DEFINE"),
        space1,
        alpha1,
        space1,
        rest,
        space0,
        eof,
    ))(i)?; 

    Ok((i, (name, text)))
}
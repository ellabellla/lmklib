use lmk_hid::key::{SpecialKey, Modifier, Key, KeyOrigin};
use nom::character::complete::{digit1, alpha1, space1, space0};
use nom::combinator::{rest, eof};
use nom::bytes::complete::{take, take_while};
use nom::multi::many1;
use nom::sequence::tuple;
use nom::{IResult, bytes::complete::tag, Parser};
use nom::branch::alt;


#[derive(Debug)]
pub enum Command<'a> {
    Rem(&'a str),
    String(&'a str),
    StringLN(&'a str),
    Special(SpecialKey),
    Modifier(Modifier),
    Shortcut(Vec<Modifier>, Key),
    Delay(Value<'a>),
    Hold(Key),
    Release(Key),
    HoldMod(Modifier),
    ReleaseMod(Modifier),
    InjectMod,
}

#[derive(Debug)]
pub enum Value<'a> {
    Int(u64),
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
     tuple((
        tag("DELAY"),
        space1,
        value
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
            key
        ))
            .map(|(_, _, key)| Command::Hold(key)),
        tuple((
            tag("RELEASE"),
            space1,
            key
        ))
            .map(|(_, _, key)| Command::Release(key)),
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
    ))(i)
}


pub fn parse_line<'a>(i: &'a str) -> IResult<&'a str, Command<'a>> {
    tuple((
        space0,
        alt((
            tuple((
                tag("REM"),
                space1,
                take_till_no_end(|c| c == '\n'),
                take_while(|c| c == '\n')
            )).map(|(_, _, str, _)| Command::Rem(str)),
            tuple((
                tag("STRING"),
                space1,
                take_till_no_end(|c| c == '\n'),
            )).map(|(_, _, str)| Command::String(str)),
            tuple((
                tag("STRINGLN"),
                space1,
                rest,
            )).map(|(_, _, str)| Command::StringLN(str)),
            tag("INJECT_MOD").map(|_| Command::InjectMod),
            delay,
            shortcut,
            special_command,
            hold_release,
            modifier_command,
            lock,
        )),
        space0,
        take_while(|c| c == '\n'),
        eof
    ))(i)
        .map(|(i, (_, command, _, _, _))| (i, command))
}


pub fn parse_define<'a>(i: &'a str) -> IResult<&'a str, (&'a str, &'a str)> {
    let (i, (_, _, _, name, _, text, _, _)) = tuple((
        space0,
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

#[cfg(test)]
mod tests {
    use crate::{parser::{parse_line}};

    #[test]
    pub fn test() {
        let input = "DELAY 100\n";

        println!("{:?}", parse_line(input).unwrap().1);
    }
}
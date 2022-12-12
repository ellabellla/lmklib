use std::{fmt::Display, sync::Arc};

use midi_msg::{MidiMsg, freq_to_midi_note_cents, ChannelVoiceMsg, midi_note_cents_to_freq};
use midir::{MidiOutput, MidiOutputConnection};
use serde::{Serialize, Deserialize};
use tokio::sync::RwLock;

use super::{Function, FunctionInterface, ReturnCommand, FunctionType};

#[derive(Debug)]
pub enum MidiError {
    NoPort,
    Init(midir::InitError),
    PortInfo(midir::PortInfoError),
    Connect(midir::ConnectError<MidiOutput>),
    Send(midir::SendError),
}

impl Display for MidiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MidiError::NoPort => f.write_str("Couldn't find a usb midi port"),
            MidiError::Init(e) => f.write_fmt(format_args!("The midi controller couldn't be initialized, {}", e)),
            MidiError::PortInfo(e) => f.write_fmt(format_args!("Port info error, {}", e)),
            MidiError::Connect(e) => f.write_fmt(format_args!("Couldn't connect to port, {}", e)),
            MidiError::Send(e) => f.write_fmt(format_args!("Couldn't send message, {}", e)),
        }
    }
}

pub struct MidiController {
    connection: Arc<RwLock<MidiOutputConnection>>,    
    last_bend: Option<u16>,
}

impl MidiController {
    pub fn new() -> Result<MidiController, MidiError>  {
        let midi_out = MidiOutput::new("LMK").map_err(|e| MidiError::Init(e))?;
        let out_ports = midi_out.ports();

        let port = 'find_port: {
            for port in &out_ports {
                if midi_out.port_name(port).map_err(|e| MidiError::PortInfo(e))?.starts_with("f_midi") {
                    break 'find_port port;
                }
            }

            return Err(MidiError::NoPort)
        };

        let connection = midi_out.connect(port, "lmk").map_err(|e| MidiError::Connect(e))?;

        Ok(MidiController{connection: Arc::new(RwLock::new(connection)), last_bend: None})
    }

    pub fn hold_note(&mut self, channel: midi_msg::Channel, note: u8, velocity: u8) -> Result<(), MidiError> {
        let msg = MidiMsg::ChannelVoice { 
            channel: channel, 
            msg: ChannelVoiceMsg::NoteOn {
                note, 
                velocity 
            } 
        };

        self.connection.blocking_write().send(&msg.to_midi()).map_err(|e| MidiError::Send(e))
    }

    pub fn release_note(&mut self, channel: midi_msg::Channel, note: u8, velocity: u8) -> Result<(), MidiError> {
        let msg = MidiMsg::ChannelVoice { 
            channel: channel, 
            msg: ChannelVoiceMsg::NoteOff {
                note, 
                velocity 
            } 
        };

        self.connection.blocking_write().send(&msg.to_midi()).map_err(|e| MidiError::Send(e))
    }

    pub fn pitch_bend(&mut self, channel: midi_msg::Channel, bend: u16) -> Result<(), MidiError> {
        self.last_bend = Some(bend);
        let msg = MidiMsg::ChannelVoice { 
            channel: channel, 
            msg: ChannelVoiceMsg::PitchBend { 
                bend
            }
        };

        self.connection.blocking_write().send(&msg.to_midi()).map_err(|e| MidiError::Send(e))
    }

    pub fn get_last_bend(&self) -> Option<u16> {
        self.last_bend
    }
}

unsafe impl Send for MidiController{}
unsafe impl Sync for MidiController{}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Channel {
    Ch1,
    Ch2,
    Ch3,
    Ch4,
    Ch5,
    Ch6,
    Ch7,
    Ch8,
    Ch9,
    Ch10,
    Ch11,
    Ch12,
    Ch13,
    Ch14,
    Ch15,
    Ch16,
}

impl Into<midi_msg::Channel> for Channel {
    fn into(self) -> midi_msg::Channel {
        match self {
            Channel::Ch1 => midi_msg::Channel::Ch1,
            Channel::Ch2 => midi_msg::Channel::Ch2,
            Channel::Ch3 => midi_msg::Channel::Ch3,
            Channel::Ch4 => midi_msg::Channel::Ch4,
            Channel::Ch5 => midi_msg::Channel::Ch5,
            Channel::Ch6 => midi_msg::Channel::Ch6,
            Channel::Ch7 => midi_msg::Channel::Ch7,
            Channel::Ch8 => midi_msg::Channel::Ch8,
            Channel::Ch9 => midi_msg::Channel::Ch9,
            Channel::Ch10 => midi_msg::Channel::Ch10,
            Channel::Ch11 => midi_msg::Channel::Ch11,
            Channel::Ch12 => midi_msg::Channel::Ch12,
            Channel::Ch13 => midi_msg::Channel::Ch13,
            Channel::Ch14 => midi_msg::Channel::Ch14,
            Channel::Ch15 => midi_msg::Channel::Ch15,
            Channel::Ch16 => midi_msg::Channel::Ch16,
        }
    }
}

impl From<midi_msg::Channel> for Channel {
    fn from(c: midi_msg::Channel) -> Self {
        match c {
            midi_msg::Channel::Ch1 => Channel::Ch1,
            midi_msg::Channel::Ch2 => Channel::Ch2,
            midi_msg::Channel::Ch3 => Channel::Ch3,
            midi_msg::Channel::Ch4 => Channel::Ch4,
            midi_msg::Channel::Ch5 => Channel::Ch5,
            midi_msg::Channel::Ch6 => Channel::Ch6,
            midi_msg::Channel::Ch7 => Channel::Ch7,
            midi_msg::Channel::Ch8 => Channel::Ch8,
            midi_msg::Channel::Ch9 => Channel::Ch9,
            midi_msg::Channel::Ch10 => Channel::Ch10,
            midi_msg::Channel::Ch11 => Channel::Ch11,
            midi_msg::Channel::Ch12 => Channel::Ch12,
            midi_msg::Channel::Ch13 => Channel::Ch13,
            midi_msg::Channel::Ch14 => Channel::Ch14,
            midi_msg::Channel::Ch15 => Channel::Ch15,
            midi_msg::Channel::Ch16 => Channel::Ch16,
        }
    }
}

pub struct Note {
    channel: midi_msg::Channel,
    note: u8,
    velocity: u8,
    prev_state: u16,
    midi_controller: Arc<RwLock<MidiController>>,
}

impl Note {
    pub fn new(channel: Channel, freq: f32, velocity: u8, midi_controller: Arc<RwLock<MidiController>>) -> Function {
        let (note, _) = freq_to_midi_note_cents(freq);
        // The velocity the note should be played at, 0-127
        let velocity = if velocity > 127 {127} else {velocity};
        Some(Box::new(Note{channel: channel.into(), note, velocity, prev_state: 0, midi_controller}))
    } 
}

impl FunctionInterface for Note {
    fn event(&mut self, state: u16) -> super::ReturnCommand {
        let mut conn = self.midi_controller.blocking_write();
        if state != 0 && self.prev_state == 0 {
            conn.hold_note(self.channel, self.note, self.velocity).ok();
        } else if state == 0 && self.prev_state != 0 {
            conn.release_note(self.channel, self.note, self.velocity).ok();
        }

        self.prev_state = state;
        ReturnCommand::None
    }

    fn ftype(&self) -> FunctionType {
        let freq = midi_note_cents_to_freq(self.note, 0.0);
        FunctionType::Note{channel: Channel::from(self.channel), freq, velocity: self.velocity}
    }
}

pub struct ConstPitchBend {
    channel: midi_msg::Channel,
    bend: u16,
    prev_state: u16,
    midi_controller: Arc<RwLock<MidiController>>,
}

impl ConstPitchBend {
    pub fn new(channel: Channel, bend: u16, midi_controller: Arc<RwLock<MidiController>>) -> Function {
        let bend = if bend > 16383 {16383} else {bend};
        Some(Box::new(ConstPitchBend{channel: channel.into(), bend, prev_state: 0, midi_controller}))
    } 
}

impl FunctionInterface for ConstPitchBend {
    fn event(&mut self, state: u16) -> super::ReturnCommand {
        let mut conn = self.midi_controller.blocking_write();
        if state != 0 && self.prev_state == 0 {
            conn.pitch_bend(self.channel, self.bend).ok();
        } else if state == 0 && self.prev_state != 0 
            && conn.get_last_bend().map(|b| b == self.bend).unwrap_or(true) {
            conn.pitch_bend(self.channel, 0).ok();
        }

        self.prev_state = state;
        ReturnCommand::None
    }

    fn ftype(&self) -> FunctionType {
        FunctionType::ConstPitchBend{channel: Channel::from(self.channel), bend: self.bend}
    }
}

pub struct PitchBend {
    channel: midi_msg::Channel,
    invert: bool,
    threshold: u16,
    scale: f64,
    midi_controller: Arc<RwLock<MidiController>>,
}

impl PitchBend {
    pub fn new(channel: Channel, invert: bool, threshold: u16, scale: f64, midi_controller: Arc<RwLock<MidiController>>) -> Function {
        Some(Box::new(PitchBend{channel: channel.into(), invert, threshold, scale, midi_controller}))
    } 
}

impl FunctionInterface for PitchBend {
    fn event(&mut self, state: u16) -> super::ReturnCommand {
        let mut conn = self.midi_controller.blocking_write();

        // Apply a pitch bend to all sounding notes. 0-8191 represent negative bends, 8192 is no bend and 8193-16383 are positive bends,
        // with the standard bend rang being +/-2 semitones per GM2

        let mut val = (state as f64) / (16383.0) * 2.0 - 1.0;

        if self.invert {
            val = -val;
        }

        val *= self.scale;
        val = if val > 1.0 {
            1.0
        } else if val < -1.0 {
            -1.0
        } else {
            val
        };

        if val < 0.0 {
            val = (1.0-val) * 8192.0;
        } else if val > 0.0 {
            val = val * (8193.0-16383.0) + 8193.0;
        };

        let bend = val as u16;
        
        conn.pitch_bend(self.channel, bend).ok();

        ReturnCommand::None
    }

    fn ftype(&self) -> FunctionType {
        FunctionType::PitchBend{ channel: Channel::from(self.channel), invert: self.invert, threshold: self.threshold, scale: self.scale}
    }
}
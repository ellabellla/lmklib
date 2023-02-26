use std::{fmt::Display, sync::Arc};

use async_trait::async_trait;
use midi_msg::{MidiMsg, ChannelVoiceMsg};
use midir::{MidiOutput};
use serde::{Serialize, Deserialize};
use tokio::{sync::{RwLock, mpsc::{UnboundedSender, self}, oneshot}};

use crate::{OrLogIgnore, OrLog, variables::{Variable}, frontend::{FrontendConfig, FrontendConfigData, FrontendConfiguration}};

use super::{Function, FunctionInterface, ReturnCommand, FunctionType, State, StateHelpers};

#[derive(Debug)]
/// Midi error
pub enum MidiError {
    /// Couldn't find port
    NoPort,
    /// Midi init error
    Init(midir::InitError),
    /// Error getting port info
    PortInfo(midir::PortInfoError),
    /// Cant connect to midi port
    Connect(midir::ConnectError<MidiOutput>),
    /// Can send midi packet
    Send(midir::SendError),
    /// Message passing error
    Channel,
}

impl Display for MidiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MidiError::NoPort => f.write_str("Couldn't find a usb midi port"),
            MidiError::Init(e) => f.write_fmt(format_args!("The midi controller couldn't be initialized, {}", e)),
            MidiError::PortInfo(e) => f.write_fmt(format_args!("Port info error, {}", e)),
            MidiError::Connect(e) => f.write_fmt(format_args!("Couldn't connect to port, {}", e)),
            MidiError::Send(e) => f.write_fmt(format_args!("Couldn't send message, {}", e)),
            MidiError::Channel => f.write_str("A channel error occurred"),
        }
    }
}

/// Midi controller
pub struct MidiController {
    last_bend: Option<u16>,
    tx: UnboundedSender<(MidiMsg, oneshot::Sender<Result<(), MidiError>>)>,
}

#[async_trait]
impl FrontendConfig for MidiController {
    type Output = Arc<RwLock<MidiController>>;
    type Error = MidiError;

    fn to_config_data(&self) -> FrontendConfigData {
        FrontendConfigData::MidiController
    }
    
    async fn from_config(_function_config: &FrontendConfiguration) -> Result<Self::Output, Self::Error> {
        MidiController::new().await
    }
}

impl MidiController {
    /// New
    pub async fn new() -> Result<Arc<RwLock<MidiController>>, MidiError> {
        let (tx, mut rx) = mpsc::unbounded_channel::<(MidiMsg, oneshot::Sender<Result<(), MidiError>>)>();
        let (new_tx, new_rx) = oneshot::channel();
        
        tokio::task::spawn_blocking(move || {
            let midi_out = match MidiOutput::new("LMK").map_err(|e| MidiError::Init(e)) {
                Ok(midi_out) => midi_out,
                Err(e) => {new_tx.send(Err(e)).or_log_ignore("Broken Channel (MIDI Driver)"); return;},
            };

            let out_ports = midi_out.ports();

            let port = 'find_port: {
                for port in &out_ports {
                    let name = match midi_out.port_name(port).map_err(|e| MidiError::PortInfo(e)) {
                        Ok(name) => name,
                        Err(e) => {new_tx.send(Err(e)).or_log_ignore("Broken Channel (MIDI Driver)"); return;},
                    };
                    if name.starts_with("f_midi") {
                        break 'find_port port;
                    }
                }

                new_tx.send(Err(MidiError::NoPort)).or_log_ignore("Broken Channel (MIDI Driver)"); 
                return 
            };

            let mut connection = match midi_out.connect(port, "lmk").map_err(|e| MidiError::Connect(e)) {
                Ok(connection) => connection,
                Err(e) => {new_tx.send(Err(e)).or_log_ignore("Broken Channel (MIDI Driver)"); return;},
            };

            new_tx.send(Ok(())).or_log_ignore("Broken Channel (MIDI Driver)");
            while let Some((msg, tx)) =  rx.blocking_recv() {
                tx.send(connection.send(&msg.to_midi()).map_err(|e| MidiError::Send(e))).or_log_ignore("Broken Channel (MIDI Driver)");

            }
        });

        if let Ok(res) = new_rx.await {
            res.map(|_| Arc::new(RwLock::new(MidiController { tx, last_bend: None })))
        } else {
            Err(MidiError::Channel)
        }
    }

    /// Send midi message
    async fn send_msg(&self, msg: MidiMsg) -> Result<(), MidiError> {
        let (tx, rx) = oneshot::channel();
        self.tx.send((msg, tx)).or_log_ignore("Broken Channel (MIDI Driver)");
        if let Ok(val) = rx.await {
            val
        } else {
            return Err(MidiError::Channel);
        }
    }

    /// Hold note
    pub async fn hold_note(&mut self, channel: midi_msg::Channel, note: u8, velocity: u8) -> Result<(), MidiError> {
        let msg = MidiMsg::ChannelVoice { 
            channel: channel, 
            msg: ChannelVoiceMsg::NoteOn {
                note, 
                velocity 
            } 
        };

        self.send_msg(msg).await        
    }

    /// Release note
    pub async fn release_note(&mut self, channel: midi_msg::Channel, note: u8, velocity: u8) -> Result<(), MidiError> {
        let msg = MidiMsg::ChannelVoice { 
            channel: channel, 
            msg: ChannelVoiceMsg::NoteOff {
                note, 
                velocity 
            } 
        };

        self.send_msg(msg).await   
    }

    /// Pitch bend
    pub async fn pitch_bend(&mut self, channel: midi_msg::Channel, bend: u16) -> Result<(), MidiError> {
        self.last_bend = Some(bend);
        let msg = MidiMsg::ChannelVoice { 
            channel: channel, 
            msg: ChannelVoiceMsg::PitchBend { 
                bend
            }
        };

        self.send_msg(msg).await   
    }

    /// Get last amount pitch bended
    pub fn get_last_bend(&self) -> Option<u16> {
        self.last_bend
    }

    /// Change instrument
    pub async fn change_instrument(&mut self, channel: midi_msg::Channel, instrument: midi_msg::GMSoundSet) -> Result<(), MidiError>  {
        let msg = MidiMsg::ChannelVoice { 
            channel: channel, 
            msg: ChannelVoiceMsg::ProgramChange { 
                program: instrument as u8
            } 
        };

        self.send_msg(msg).await   
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
/// Midi channel
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

#[derive(Debug, Clone, Serialize, Deserialize)]
/// Instrument
pub enum GMSoundSet {
    AcousticGrandPiano,
    BrightAcousticPiano,
    ElectricGrandPiano,
    HonkytonkPiano,
    ElectricPiano1,
    ElectricPiano2,
    Harpsichord,
    Clavi,
    Celesta,
    Glockenspiel,
    MusicBox,
    Vibraphone,
    Marimba,
    Xylophone,
    TubularBells,
    Dulcimer,
    DrawbarOrgan,
    PercussiveOrgan,
    RockOrgan,
    ChurchOrgan,
    ReedOrgan,
    Accordion,
    Harmonica,
    TangoAccordion,
    AcousticGuitarNylon,
    AcousticGuitarSteel,
    ElectricGuitarJazz,
    ElectricGuitarClean,
    ElectricGuitarMuted,
    OverdrivenGuitar,
    DistortionGuitar,
    GuitarHarmonics,
    AcousticBass,
    ElectricBassFinger,
    ElectricBassPick,
    FretlessBass,
    SlapBass1,
    SlapBass2,
    SynthBass1,
    SynthBass2,
    Violin,
    Viola,
    Cello,
    Contrabass,
    TremoloStrings,
    PizzicatoStrings,
    OrchestralHarp,
    Timpani,
    StringEnsemble1,
    StringEnsemble2,
    SynthStrings1,
    SynthStrings2,
    ChoirAahs,
    VoiceOohs,
    SynthVoice,
    OrchestraHit,
    Trumpet,
    Trombone,
    Tuba,
    MutedTrumpet,
    FrenchHorn,
    BrassSection,
    SynthBrass1,
    SynthBrass2,
    SopranoSax,
    AltoSax,
    TenorSax,
    BaritoneSax,
    Oboe,
    EnglishHorn,
    Bassoon,
    Clarinet,
    Piccolo,
    Flute,
    Recorder,
    PanFlute,
    BlownBottle,
    Shakuhachi,
    Whistle,
    Ocarina,
    Lead1,
    Lead2,
    Lead3,
    Lead4,
    Lead5,
    Lead6,
    Lead7,
    Lead8,
    Pad1,
    Pad2,
    Pad3,
    Pad4,
    Pad5,
    Pad6,
    Pad7,
    Pad8,
    FX1,
    FX2,
    FX3,
    FX4,
    FX5,
    FX6,
    FX7,
    FX8,
    Sitar,
    Banjo,
    Shamisen,
    Koto,
    Kalimba,
    Bagpipe,
    Fiddle,
    Shanai,
    TinkleBell,
    Agogo,
    SteelDrums,
    Woodblock,
    TaikoDrum,
    MelodicTom,
    SynthDrum,
    ReverseCymbal,
    GuitarFretNoise,
    BreathNoise,
    Seashore,
    BirdTweet,
    TelephoneRing,
    Helicopter,
    Applause,
    Gunshot,
}

impl Into<midi_msg::GMSoundSet> for GMSoundSet {
    fn into(self) -> midi_msg::GMSoundSet {
        match self {
            GMSoundSet::AcousticGrandPiano => midi_msg::GMSoundSet::AcousticGrandPiano,
            GMSoundSet::BrightAcousticPiano => midi_msg::GMSoundSet::BrightAcousticPiano,
            GMSoundSet::ElectricGrandPiano => midi_msg::GMSoundSet::ElectricGrandPiano,
            GMSoundSet::HonkytonkPiano => midi_msg::GMSoundSet::HonkytonkPiano,
            GMSoundSet::ElectricPiano1 => midi_msg::GMSoundSet::ElectricPiano1,
            GMSoundSet::ElectricPiano2 => midi_msg::GMSoundSet::ElectricPiano2,
            GMSoundSet::Harpsichord => midi_msg::GMSoundSet::Harpsichord,
            GMSoundSet::Clavi => midi_msg::GMSoundSet::Clavi,
            GMSoundSet::Celesta => midi_msg::GMSoundSet::Celesta,
            GMSoundSet::Glockenspiel => midi_msg::GMSoundSet::Glockenspiel,
            GMSoundSet::MusicBox => midi_msg::GMSoundSet::MusicBox,
            GMSoundSet::Vibraphone => midi_msg::GMSoundSet::Vibraphone,
            GMSoundSet::Marimba => midi_msg::GMSoundSet::Marimba,
            GMSoundSet::Xylophone => midi_msg::GMSoundSet::Xylophone,
            GMSoundSet::TubularBells => midi_msg::GMSoundSet::TubularBells,
            GMSoundSet::Dulcimer => midi_msg::GMSoundSet::Dulcimer,
            GMSoundSet::DrawbarOrgan => midi_msg::GMSoundSet::DrawbarOrgan,
            GMSoundSet::PercussiveOrgan => midi_msg::GMSoundSet::PercussiveOrgan,
            GMSoundSet::RockOrgan => midi_msg::GMSoundSet::RockOrgan,
            GMSoundSet::ChurchOrgan => midi_msg::GMSoundSet::ChurchOrgan,
            GMSoundSet::ReedOrgan => midi_msg::GMSoundSet::ReedOrgan,
            GMSoundSet::Accordion => midi_msg::GMSoundSet::Accordion,
            GMSoundSet::Harmonica => midi_msg::GMSoundSet::Harmonica,
            GMSoundSet::TangoAccordion => midi_msg::GMSoundSet::TangoAccordion,
            GMSoundSet::AcousticGuitarNylon => midi_msg::GMSoundSet::AcousticGuitarNylon,
            GMSoundSet::AcousticGuitarSteel => midi_msg::GMSoundSet::AcousticGuitarSteel,
            GMSoundSet::ElectricGuitarJazz => midi_msg::GMSoundSet::ElectricGuitarJazz,
            GMSoundSet::ElectricGuitarClean => midi_msg::GMSoundSet::ElectricGuitarClean,
            GMSoundSet::ElectricGuitarMuted => midi_msg::GMSoundSet::ElectricGuitarMuted,
            GMSoundSet::OverdrivenGuitar => midi_msg::GMSoundSet::OverdrivenGuitar,
            GMSoundSet::DistortionGuitar => midi_msg::GMSoundSet::DistortionGuitar,
            GMSoundSet::GuitarHarmonics => midi_msg::GMSoundSet::GuitarHarmonics,
            GMSoundSet::AcousticBass => midi_msg::GMSoundSet::AcousticBass,
            GMSoundSet::ElectricBassFinger => midi_msg::GMSoundSet::ElectricBassFinger,
            GMSoundSet::ElectricBassPick => midi_msg::GMSoundSet::ElectricBassPick,
            GMSoundSet::FretlessBass => midi_msg::GMSoundSet::FretlessBass,
            GMSoundSet::SlapBass1 => midi_msg::GMSoundSet::SlapBass1,
            GMSoundSet::SlapBass2 => midi_msg::GMSoundSet::SlapBass2,
            GMSoundSet::SynthBass1 => midi_msg::GMSoundSet::SynthBass1,
            GMSoundSet::SynthBass2 => midi_msg::GMSoundSet::SynthBass2,
            GMSoundSet::Violin => midi_msg::GMSoundSet::Violin,
            GMSoundSet::Viola => midi_msg::GMSoundSet::Viola,
            GMSoundSet::Cello => midi_msg::GMSoundSet::Cello,
            GMSoundSet::Contrabass => midi_msg::GMSoundSet::Contrabass,
            GMSoundSet::TremoloStrings => midi_msg::GMSoundSet::TremoloStrings,
            GMSoundSet::PizzicatoStrings => midi_msg::GMSoundSet::PizzicatoStrings,
            GMSoundSet::OrchestralHarp => midi_msg::GMSoundSet::OrchestralHarp,
            GMSoundSet::Timpani => midi_msg::GMSoundSet::Timpani,
            GMSoundSet::StringEnsemble1 => midi_msg::GMSoundSet::StringEnsemble1,
            GMSoundSet::StringEnsemble2 => midi_msg::GMSoundSet::StringEnsemble2,
            GMSoundSet::SynthStrings1 => midi_msg::GMSoundSet::SynthStrings1,
            GMSoundSet::SynthStrings2 => midi_msg::GMSoundSet::SynthStrings2,
            GMSoundSet::ChoirAahs => midi_msg::GMSoundSet::ChoirAahs,
            GMSoundSet::VoiceOohs => midi_msg::GMSoundSet::VoiceOohs,
            GMSoundSet::SynthVoice => midi_msg::GMSoundSet::SynthVoice,
            GMSoundSet::OrchestraHit => midi_msg::GMSoundSet::OrchestraHit,
            GMSoundSet::Trumpet => midi_msg::GMSoundSet::Trumpet,
            GMSoundSet::Trombone => midi_msg::GMSoundSet::Trombone,
            GMSoundSet::Tuba => midi_msg::GMSoundSet::Tuba,
            GMSoundSet::MutedTrumpet => midi_msg::GMSoundSet::MutedTrumpet,
            GMSoundSet::FrenchHorn => midi_msg::GMSoundSet::FrenchHorn,
            GMSoundSet::BrassSection => midi_msg::GMSoundSet::BrassSection,
            GMSoundSet::SynthBrass1 => midi_msg::GMSoundSet::SynthBrass1,
            GMSoundSet::SynthBrass2 => midi_msg::GMSoundSet::SynthBrass2,
            GMSoundSet::SopranoSax => midi_msg::GMSoundSet::SopranoSax,
            GMSoundSet::AltoSax => midi_msg::GMSoundSet::AltoSax,
            GMSoundSet::TenorSax => midi_msg::GMSoundSet::TenorSax,
            GMSoundSet::BaritoneSax => midi_msg::GMSoundSet::BaritoneSax,
            GMSoundSet::Oboe => midi_msg::GMSoundSet::Oboe,
            GMSoundSet::EnglishHorn => midi_msg::GMSoundSet::EnglishHorn,
            GMSoundSet::Bassoon => midi_msg::GMSoundSet::Bassoon,
            GMSoundSet::Clarinet => midi_msg::GMSoundSet::Clarinet,
            GMSoundSet::Piccolo => midi_msg::GMSoundSet::Piccolo,
            GMSoundSet::Flute => midi_msg::GMSoundSet::Flute,
            GMSoundSet::Recorder => midi_msg::GMSoundSet::Recorder,
            GMSoundSet::PanFlute => midi_msg::GMSoundSet::PanFlute,
            GMSoundSet::BlownBottle => midi_msg::GMSoundSet::BlownBottle,
            GMSoundSet::Shakuhachi => midi_msg::GMSoundSet::Shakuhachi,
            GMSoundSet::Whistle => midi_msg::GMSoundSet::Whistle,
            GMSoundSet::Ocarina => midi_msg::GMSoundSet::Ocarina,
            GMSoundSet::Lead1 => midi_msg::GMSoundSet::Lead1,
            GMSoundSet::Lead2 => midi_msg::GMSoundSet::Lead2,
            GMSoundSet::Lead3 => midi_msg::GMSoundSet::Lead3,
            GMSoundSet::Lead4 => midi_msg::GMSoundSet::Lead4,
            GMSoundSet::Lead5 => midi_msg::GMSoundSet::Lead5,
            GMSoundSet::Lead6 => midi_msg::GMSoundSet::Lead6,
            GMSoundSet::Lead7 => midi_msg::GMSoundSet::Lead7,
            GMSoundSet::Lead8 => midi_msg::GMSoundSet::Lead8,
            GMSoundSet::Pad1 => midi_msg::GMSoundSet::Pad1,
            GMSoundSet::Pad2 => midi_msg::GMSoundSet::Pad2,
            GMSoundSet::Pad3 => midi_msg::GMSoundSet::Pad3,
            GMSoundSet::Pad4 => midi_msg::GMSoundSet::Pad4,
            GMSoundSet::Pad5 => midi_msg::GMSoundSet::Pad5,
            GMSoundSet::Pad6 => midi_msg::GMSoundSet::Pad6,
            GMSoundSet::Pad7 => midi_msg::GMSoundSet::Pad7,
            GMSoundSet::Pad8 => midi_msg::GMSoundSet::Pad8,
            GMSoundSet::FX1 => midi_msg::GMSoundSet::FX1,
            GMSoundSet::FX2 => midi_msg::GMSoundSet::FX2,
            GMSoundSet::FX3 => midi_msg::GMSoundSet::FX3,
            GMSoundSet::FX4 => midi_msg::GMSoundSet::FX4,
            GMSoundSet::FX5 => midi_msg::GMSoundSet::FX5,
            GMSoundSet::FX6 => midi_msg::GMSoundSet::FX6,
            GMSoundSet::FX7 => midi_msg::GMSoundSet::FX7,
            GMSoundSet::FX8 => midi_msg::GMSoundSet::FX8,
            GMSoundSet::Sitar => midi_msg::GMSoundSet::Sitar,
            GMSoundSet::Banjo => midi_msg::GMSoundSet::Banjo,
            GMSoundSet::Shamisen => midi_msg::GMSoundSet::Shamisen,
            GMSoundSet::Koto => midi_msg::GMSoundSet::Koto,
            GMSoundSet::Kalimba => midi_msg::GMSoundSet::Kalimba,
            GMSoundSet::Bagpipe => midi_msg::GMSoundSet::Bagpipe,
            GMSoundSet::Fiddle => midi_msg::GMSoundSet::Fiddle,
            GMSoundSet::Shanai => midi_msg::GMSoundSet::Shanai,
            GMSoundSet::TinkleBell => midi_msg::GMSoundSet::TinkleBell,
            GMSoundSet::Agogo => midi_msg::GMSoundSet::Agogo,
            GMSoundSet::SteelDrums => midi_msg::GMSoundSet::SteelDrums,
            GMSoundSet::Woodblock => midi_msg::GMSoundSet::Woodblock,
            GMSoundSet::TaikoDrum => midi_msg::GMSoundSet::TaikoDrum,
            GMSoundSet::MelodicTom => midi_msg::GMSoundSet::MelodicTom,
            GMSoundSet::SynthDrum => midi_msg::GMSoundSet::SynthDrum,
            GMSoundSet::ReverseCymbal => midi_msg::GMSoundSet::ReverseCymbal,
            GMSoundSet::GuitarFretNoise => midi_msg::GMSoundSet::GuitarFretNoise,
            GMSoundSet::BreathNoise => midi_msg::GMSoundSet::BreathNoise,
            GMSoundSet::Seashore => midi_msg::GMSoundSet::Seashore,
            GMSoundSet::BirdTweet => midi_msg::GMSoundSet::BirdTweet,
            GMSoundSet::TelephoneRing => midi_msg::GMSoundSet::TelephoneRing,
            GMSoundSet::Helicopter => midi_msg::GMSoundSet::Helicopter,
            GMSoundSet::Applause => midi_msg::GMSoundSet::Applause,
            GMSoundSet::Gunshot => midi_msg::GMSoundSet::Gunshot,
        }
    }
}

impl From<midi_msg::GMSoundSet> for GMSoundSet {
    fn from(i: midi_msg::GMSoundSet) -> Self {
        match i {
            midi_msg::GMSoundSet::AcousticGrandPiano => GMSoundSet::AcousticGrandPiano,
            midi_msg::GMSoundSet::BrightAcousticPiano => GMSoundSet::BrightAcousticPiano,
            midi_msg::GMSoundSet::ElectricGrandPiano => GMSoundSet::ElectricGrandPiano,
            midi_msg::GMSoundSet::HonkytonkPiano => GMSoundSet::HonkytonkPiano,
            midi_msg::GMSoundSet::ElectricPiano1 => GMSoundSet::ElectricPiano1,
            midi_msg::GMSoundSet::ElectricPiano2 => GMSoundSet::ElectricPiano2,
            midi_msg::GMSoundSet::Harpsichord => GMSoundSet::Harpsichord,
            midi_msg::GMSoundSet::Clavi => GMSoundSet::Clavi,
            midi_msg::GMSoundSet::Celesta => GMSoundSet::Celesta,
            midi_msg::GMSoundSet::Glockenspiel => GMSoundSet::Glockenspiel,
            midi_msg::GMSoundSet::MusicBox => GMSoundSet::MusicBox,
            midi_msg::GMSoundSet::Vibraphone => GMSoundSet::Vibraphone,
            midi_msg::GMSoundSet::Marimba => GMSoundSet::Marimba,
            midi_msg::GMSoundSet::Xylophone => GMSoundSet::Xylophone,
            midi_msg::GMSoundSet::TubularBells => GMSoundSet::TubularBells,
            midi_msg::GMSoundSet::Dulcimer => GMSoundSet::Dulcimer,
            midi_msg::GMSoundSet::DrawbarOrgan => GMSoundSet::DrawbarOrgan,
            midi_msg::GMSoundSet::PercussiveOrgan => GMSoundSet::PercussiveOrgan,
            midi_msg::GMSoundSet::RockOrgan => GMSoundSet::RockOrgan,
            midi_msg::GMSoundSet::ChurchOrgan => GMSoundSet::ChurchOrgan,
            midi_msg::GMSoundSet::ReedOrgan => GMSoundSet::ReedOrgan,
            midi_msg::GMSoundSet::Accordion => GMSoundSet::Accordion,
            midi_msg::GMSoundSet::Harmonica => GMSoundSet::Harmonica,
            midi_msg::GMSoundSet::TangoAccordion => GMSoundSet::TangoAccordion,
            midi_msg::GMSoundSet::AcousticGuitarNylon => GMSoundSet::AcousticGuitarNylon,
            midi_msg::GMSoundSet::AcousticGuitarSteel => GMSoundSet::AcousticGuitarSteel,
            midi_msg::GMSoundSet::ElectricGuitarJazz => GMSoundSet::ElectricGuitarJazz,
            midi_msg::GMSoundSet::ElectricGuitarClean => GMSoundSet::ElectricGuitarClean,
            midi_msg::GMSoundSet::ElectricGuitarMuted => GMSoundSet::ElectricGuitarMuted,
            midi_msg::GMSoundSet::OverdrivenGuitar => GMSoundSet::OverdrivenGuitar,
            midi_msg::GMSoundSet::DistortionGuitar => GMSoundSet::DistortionGuitar,
            midi_msg::GMSoundSet::GuitarHarmonics => GMSoundSet::GuitarHarmonics,
            midi_msg::GMSoundSet::AcousticBass => GMSoundSet::AcousticBass,
            midi_msg::GMSoundSet::ElectricBassFinger => GMSoundSet::ElectricBassFinger,
            midi_msg::GMSoundSet::ElectricBassPick => GMSoundSet::ElectricBassPick,
            midi_msg::GMSoundSet::FretlessBass => GMSoundSet::FretlessBass,
            midi_msg::GMSoundSet::SlapBass1 => GMSoundSet::SlapBass1,
            midi_msg::GMSoundSet::SlapBass2 => GMSoundSet::SlapBass2,
            midi_msg::GMSoundSet::SynthBass1 => GMSoundSet::SynthBass1,
            midi_msg::GMSoundSet::SynthBass2 => GMSoundSet::SynthBass2,
            midi_msg::GMSoundSet::Violin => GMSoundSet::Violin,
            midi_msg::GMSoundSet::Viola => GMSoundSet::Viola,
            midi_msg::GMSoundSet::Cello => GMSoundSet::Cello,
            midi_msg::GMSoundSet::Contrabass => GMSoundSet::Contrabass,
            midi_msg::GMSoundSet::TremoloStrings => GMSoundSet::TremoloStrings,
            midi_msg::GMSoundSet::PizzicatoStrings => GMSoundSet::PizzicatoStrings,
            midi_msg::GMSoundSet::OrchestralHarp => GMSoundSet::OrchestralHarp,
            midi_msg::GMSoundSet::Timpani => GMSoundSet::Timpani,
            midi_msg::GMSoundSet::StringEnsemble1 => GMSoundSet::StringEnsemble1,
            midi_msg::GMSoundSet::StringEnsemble2 => GMSoundSet::StringEnsemble2,
            midi_msg::GMSoundSet::SynthStrings1 => GMSoundSet::SynthStrings1,
            midi_msg::GMSoundSet::SynthStrings2 => GMSoundSet::SynthStrings2,
            midi_msg::GMSoundSet::ChoirAahs => GMSoundSet::ChoirAahs,
            midi_msg::GMSoundSet::VoiceOohs => GMSoundSet::VoiceOohs,
            midi_msg::GMSoundSet::SynthVoice => GMSoundSet::SynthVoice,
            midi_msg::GMSoundSet::OrchestraHit => GMSoundSet::OrchestraHit,
            midi_msg::GMSoundSet::Trumpet => GMSoundSet::Trumpet,
            midi_msg::GMSoundSet::Trombone => GMSoundSet::Trombone,
            midi_msg::GMSoundSet::Tuba => GMSoundSet::Tuba,
            midi_msg::GMSoundSet::MutedTrumpet => GMSoundSet::MutedTrumpet,
            midi_msg::GMSoundSet::FrenchHorn => GMSoundSet::FrenchHorn,
            midi_msg::GMSoundSet::BrassSection => GMSoundSet::BrassSection,
            midi_msg::GMSoundSet::SynthBrass1 => GMSoundSet::SynthBrass1,
            midi_msg::GMSoundSet::SynthBrass2 => GMSoundSet::SynthBrass2,
            midi_msg::GMSoundSet::SopranoSax => GMSoundSet::SopranoSax,
            midi_msg::GMSoundSet::AltoSax => GMSoundSet::AltoSax,
            midi_msg::GMSoundSet::TenorSax => GMSoundSet::TenorSax,
            midi_msg::GMSoundSet::BaritoneSax => GMSoundSet::BaritoneSax,
            midi_msg::GMSoundSet::Oboe => GMSoundSet::Oboe,
            midi_msg::GMSoundSet::EnglishHorn => GMSoundSet::EnglishHorn,
            midi_msg::GMSoundSet::Bassoon => GMSoundSet::Bassoon,
            midi_msg::GMSoundSet::Clarinet => GMSoundSet::Clarinet,
            midi_msg::GMSoundSet::Piccolo => GMSoundSet::Piccolo,
            midi_msg::GMSoundSet::Flute => GMSoundSet::Flute,
            midi_msg::GMSoundSet::Recorder => GMSoundSet::Recorder,
            midi_msg::GMSoundSet::PanFlute => GMSoundSet::PanFlute,
            midi_msg::GMSoundSet::BlownBottle => GMSoundSet::BlownBottle,
            midi_msg::GMSoundSet::Shakuhachi => GMSoundSet::Shakuhachi,
            midi_msg::GMSoundSet::Whistle => GMSoundSet::Whistle,
            midi_msg::GMSoundSet::Ocarina => GMSoundSet::Ocarina,
            midi_msg::GMSoundSet::Lead1 => GMSoundSet::Lead1,
            midi_msg::GMSoundSet::Lead2 => GMSoundSet::Lead2,
            midi_msg::GMSoundSet::Lead3 => GMSoundSet::Lead3,
            midi_msg::GMSoundSet::Lead4 => GMSoundSet::Lead4,
            midi_msg::GMSoundSet::Lead5 => GMSoundSet::Lead5,
            midi_msg::GMSoundSet::Lead6 => GMSoundSet::Lead6,
            midi_msg::GMSoundSet::Lead7 => GMSoundSet::Lead7,
            midi_msg::GMSoundSet::Lead8 => GMSoundSet::Lead8,
            midi_msg::GMSoundSet::Pad1 => GMSoundSet::Pad1,
            midi_msg::GMSoundSet::Pad2 => GMSoundSet::Pad2,
            midi_msg::GMSoundSet::Pad3 => GMSoundSet::Pad3,
            midi_msg::GMSoundSet::Pad4 => GMSoundSet::Pad4,
            midi_msg::GMSoundSet::Pad5 => GMSoundSet::Pad5,
            midi_msg::GMSoundSet::Pad6 => GMSoundSet::Pad6,
            midi_msg::GMSoundSet::Pad7 => GMSoundSet::Pad7,
            midi_msg::GMSoundSet::Pad8 => GMSoundSet::Pad8,
            midi_msg::GMSoundSet::FX1 => GMSoundSet::FX1,
            midi_msg::GMSoundSet::FX2 => GMSoundSet::FX2,
            midi_msg::GMSoundSet::FX3 => GMSoundSet::FX3,
            midi_msg::GMSoundSet::FX4 => GMSoundSet::FX4,
            midi_msg::GMSoundSet::FX5 => GMSoundSet::FX5,
            midi_msg::GMSoundSet::FX6 => GMSoundSet::FX6,
            midi_msg::GMSoundSet::FX7 => GMSoundSet::FX7,
            midi_msg::GMSoundSet::FX8 => GMSoundSet::FX8,
            midi_msg::GMSoundSet::Sitar => GMSoundSet::Sitar,
            midi_msg::GMSoundSet::Banjo => GMSoundSet::Banjo,
            midi_msg::GMSoundSet::Shamisen => GMSoundSet::Shamisen,
            midi_msg::GMSoundSet::Koto => GMSoundSet::Koto,
            midi_msg::GMSoundSet::Kalimba => GMSoundSet::Kalimba,
            midi_msg::GMSoundSet::Bagpipe => GMSoundSet::Bagpipe,
            midi_msg::GMSoundSet::Fiddle => GMSoundSet::Fiddle,
            midi_msg::GMSoundSet::Shanai => GMSoundSet::Shanai,
            midi_msg::GMSoundSet::TinkleBell => GMSoundSet::TinkleBell,
            midi_msg::GMSoundSet::Agogo => GMSoundSet::Agogo,
            midi_msg::GMSoundSet::SteelDrums => GMSoundSet::SteelDrums,
            midi_msg::GMSoundSet::Woodblock => GMSoundSet::Woodblock,
            midi_msg::GMSoundSet::TaikoDrum => GMSoundSet::TaikoDrum,
            midi_msg::GMSoundSet::MelodicTom => GMSoundSet::MelodicTom,
            midi_msg::GMSoundSet::SynthDrum => GMSoundSet::SynthDrum,
            midi_msg::GMSoundSet::ReverseCymbal => GMSoundSet::ReverseCymbal,
            midi_msg::GMSoundSet::GuitarFretNoise => GMSoundSet::GuitarFretNoise,
            midi_msg::GMSoundSet::BreathNoise => GMSoundSet::BreathNoise,
            midi_msg::GMSoundSet::Seashore => GMSoundSet::Seashore,
            midi_msg::GMSoundSet::BirdTweet => GMSoundSet::BirdTweet,
            midi_msg::GMSoundSet::TelephoneRing => GMSoundSet::TelephoneRing,
            midi_msg::GMSoundSet::Helicopter => GMSoundSet::Helicopter,
            midi_msg::GMSoundSet::Applause => GMSoundSet::Applause,
            midi_msg::GMSoundSet::Gunshot => GMSoundSet::Gunshot,
        }
    }
}

/// Note function, play a note
pub struct Note {
    channel: Variable<Channel>,
    note: Variable<u8>,
    velocity: Variable<u8>,
    prev_state: u16,
    midi_controller: Arc<RwLock<MidiController>>,
}

impl Note {
    /// New
    pub fn new(channel: Variable<Channel>, note: Variable<note_param::Note>, velocity: Variable<u8>, midi_controller: Arc<RwLock<MidiController>>) -> Function {
        let note = note.map(|n| n.to_note());
        // The velocity the note should be played at, 0-127
        let velocity = velocity.map(|velocity| if velocity > 127 {127} else {velocity});

        Some(Box::new(Note{channel, note, velocity, prev_state: 0, midi_controller}))
    } 
}

#[async_trait]
impl FunctionInterface for Note {
    async fn event(&mut self, state: State) -> super::ReturnCommand {
        let mut conn = self.midi_controller.write().await;

        if state.rising(self.prev_state) {
            conn.hold_note(self.channel.data().to_owned().into(), *self.note.data(), *self.velocity.data()).await.or_log("MIDI error (MIDI Driver)");
        } else if state.falling(self.prev_state) {
            conn.release_note(self.channel.data().to_owned().into(), *self.note.data(), *self.velocity.data()).await.or_log("MIDI error (MIDI Driver)");
        }

        self.prev_state = state;
        ReturnCommand::None
    }

    fn ftype(&self) -> FunctionType {
        let note = self.note.clone().map(|n| note_param::Note::from_note(n));

        FunctionType::Note{channel: self.channel.into_data(), note: note.into_data(), velocity: self.velocity.into_data()}
    }
}

/// Const Pitch Bend, bend pitch by a constant amount when pressed
pub struct ConstPitchBend {
    channel: Variable<Channel>,
    bend: Variable<u16>,
    prev_state: u16,
    midi_controller: Arc<RwLock<MidiController>>,
}

impl ConstPitchBend {
    /// New
    pub fn new(channel: Variable<Channel>, bend: Variable<u16>, midi_controller: Arc<RwLock<MidiController>>) -> Function {
        let bend = bend.map(|bend| if bend > 16383 {16383} else {bend});

        Some(Box::new(ConstPitchBend{channel: channel, bend, prev_state: 0, midi_controller}))
    } 
}

#[async_trait]
impl FunctionInterface for ConstPitchBend {
    async fn event(&mut self, state: State) -> super::ReturnCommand {
        let mut conn = self.midi_controller.write().await;

        if state.rising(self.prev_state) {
            conn.pitch_bend(self.channel.data().to_owned().into(), *self.bend.data()).await.or_log("MIDI error (MIDI Driver)");
        } else if state.falling(self.prev_state) 
            && conn.get_last_bend().map(|b| b == *self.bend.data()).unwrap_or(true) {
            conn.pitch_bend(self.channel.data().to_owned().into(), 0).await.or_log("MIDI error (MIDI Driver)");
        }

        self.prev_state = state;
        ReturnCommand::None
    }

    fn ftype(&self) -> FunctionType {
        FunctionType::ConstPitchBend{channel: self.channel.into_data(), bend: self.bend.into_data()}
    }
}

/// Pitch Bend function, bend pitch based on state
pub struct PitchBend {
    channel: Variable<Channel>,
    invert: Variable<bool>,
    threshold: Variable<u16>,
    scale: Variable<f64>,
    midi_controller: Arc<RwLock<MidiController>>,
}

impl PitchBend {
    /// New 
    pub fn new(channel: Variable<Channel>, invert: Variable<bool>, threshold: Variable<u16>, scale: Variable<f64>, midi_controller: Arc<RwLock<MidiController>>) -> Function {

        Some(Box::new(PitchBend{channel, invert, threshold, scale, midi_controller}))
    } 
}

#[async_trait]
impl FunctionInterface for PitchBend {
    async fn event(&mut self, state: State) -> super::ReturnCommand {
        let mut conn = self.midi_controller.write().await;

        // Apply a pitch bend to all sounding notes. 0-8191 represent negative bends, 8192 is no bend and 8193-16383 are positive bends,
        // with the standard bend rang being +/-2 semitones per GM2

        let mut val = (state as f64) / (16383.0) * 2.0 - 1.0;

        if *self.invert.data() {
            val = -val;
        }

        val *= self.scale.data();
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
        
        conn.pitch_bend(self.channel.data().to_owned().into(), bend).await.or_log("MIDI error (MIDI Driver)");

        ReturnCommand::None
    }

    fn ftype(&self) -> FunctionType {
        FunctionType::PitchBend{ channel: self.channel.into_data(), invert: self.invert.into_data(), threshold: self.threshold.into_data(), scale: self.scale.into_data()}
    }
}


/// Instrument function, set instrument
pub struct Instrument {
    channel: Variable<Channel>,
    instrument: Variable<GMSoundSet>,
    prev_state: u16,
    midi_controller: Arc<RwLock<MidiController>>,
}

impl Instrument {
    pub fn new(channel: Variable<Channel>, instrument: Variable<GMSoundSet>, midi_controller: Arc<RwLock<MidiController>>) -> Function {
        Some(Box::new(Instrument{channel, instrument, prev_state: 0, midi_controller}))
    } 
}

#[async_trait]
impl FunctionInterface for Instrument {
    async fn event(&mut self, state: State) -> super::ReturnCommand {
        let mut conn = self.midi_controller.write().await;

        if state.rising(self.prev_state) {
            conn.change_instrument(self.channel.data().to_owned().into(), self.instrument.data().to_owned().into()).await.or_log("MIDI error (MIDI Driver)");
        }

        self.prev_state = state;
        ReturnCommand::None
    }

    fn ftype(&self) -> FunctionType {
        FunctionType::Instrument{channel: self.channel.into_data(), instrument: self.instrument.into_data()}
    }
}

pub mod note_param {
    use midi_msg::freq_to_midi_note_cents;
    use serde::{Serialize, Deserialize};

    #[derive(Debug, Clone, Serialize, Deserialize)]
    /// Serializable note, used in config
    pub enum Note {
        G9,
        FS9,
        F9,
        E9,
        DS9,
        D9,
        CS9,
        C9,
        B8,
        AS8,
        A8,
        GS8,
        G8,
        FS8,
        F8,
        E8,
        DS8,
        D8,
        CS8,
        C8,
        B7,
        AS7,
        A7,
        GS7,
        G7,
        FS7,
        F7,
        E7,
        DS7,
        D7,
        CS7,
        C7,
        B6,
        AS6,
        A6,
        GS6,
        G6,
        FS6,
        F6,
        E6,
        DS6,
        D6,
        CS6,
        C6,
        B5,
        AS5,
        A5,
        GS5,
        G5,
        FS5,
        F5,
        E5,
        DS5,
        D5,
        CS5,
        C5,
        B4,
        AS4,
        A4,
        GS4,
        G4,
        FS4,
        F4,
        E4,
        DS4,
        D4,
        CS4,
        C4,
        B3,
        AS3,
        A3,
        GS3,
        G3,
        FS3,
        F3,
        E3,
        DS3,
        D3,
        CS3,
        C3,
        B2,
        AS2,
        A2,
        GS2,
        G2,
        FS2,
        F2,
        E2,
        DS2,
        D2,
        CS2,
        C2,
        B1,
        AS1,
        A1,
        GS1,
        G1,
        FS1,
        F1,
        E1,
        DS1,
        D1,
        CS1,
        C1,
        B0,
        AS0,
        A0,
        _20,
        _19,
        _18,
        _17,
        _16,
        _15,
        _14,
        _13,
        _12,
        _11,
        _10,
        _9,
        _8,
        _7,
        _6,
        _5,
        _4,
        _3,
        _2,
        _1,
        _0,
        AcousticBassDrum,
        RideCymbal1,
        HighAgogo,
        BassDrum1,
        ChineseCymbal,
        LowAgogo,
        SideStick,
        RideBell,
        Cabasa,
        AcousticSnare,
        Tambourine,
        Maracas,
        HandClap,
        SplashCymbal,
        ShortWhistle,
        ElectricSnare,
        Cowbell,
        LongWhistle,
        LowFloorTom,
        CrashCymbal2,
        ShortGuiro,
        ClosedHiHat,
        Vibraslap,
        LongGuiro,
        HighFloorTom,
        RideCymbal2,
        Claves,
        PedalHiHat,
        HiBongo,
        HiWoodBlock,
        LowTom,
        LowBongo,
        LowWoodBlock,
        OpenHiHat,
        MuteHiConga,
        MuteCuica,
        LowMidTom,
        OpenHiConga,
        OpenCuica,
        HiMidTom,
        LowConga,
        MuteTriangle,
        CrashCymbal1,
        HighTimbale,
        OpenTriangle,
        HighTom,
        LowTimbale,
        Freq(f32),
    }

    impl Note {
        /// Convert to midi note
        pub fn to_note(&self) -> u8 {
            match self {
                Note::G9 => 127,
                Note::FS9 => 126,
                Note::F9 => 125,
                Note::E9 => 124,
                Note::DS9 => 123,
                Note::D9 => 122,
                Note::CS9 => 121,
                Note::C9 => 120,
                Note::B8 => 119,
                Note::AS8 => 118,
                Note::A8 => 117,
                Note::GS8 => 116,
                Note::G8 => 115,
                Note::FS8 => 114,
                Note::F8 => 113,
                Note::E8 => 112,
                Note::DS8 => 111,
                Note::D8 => 110,
                Note::CS8 => 109,
                Note::C8 => 108,
                Note::B7 => 107,
                Note::AS7 => 106,
                Note::A7 => 105,
                Note::GS7 => 104,
                Note::G7 => 103,
                Note::FS7 => 102,
                Note::F7 => 101,
                Note::E7 => 100,
                Note::DS7 => 99,
                Note::D7 => 98,
                Note::CS7 => 97,
                Note::C7 => 96,
                Note::B6 => 95,
                Note::AS6 => 94,
                Note::A6 => 93,
                Note::GS6 => 92,
                Note::G6 => 91,
                Note::FS6 => 90,
                Note::F6 => 89,
                Note::E6 => 88,
                Note::DS6 => 87,
                Note::D6 => 86,
                Note::CS6 => 85,
                Note::C6 => 84,
                Note::B5 => 83,
                Note::AS5 => 82,
                Note::A5 => 81,
                Note::GS5 => 80,
                Note::G5 => 79,
                Note::FS5 => 78,
                Note::F5 => 77,
                Note::E5 => 76,
                Note::DS5 => 75,
                Note::D5 => 74,
                Note::CS5 => 73,
                Note::C5 => 72,
                Note::B4 => 71,
                Note::AS4 => 70,
                Note::A4 => 69,
                Note::GS4 => 68,
                Note::G4 => 67,
                Note::FS4 => 66,
                Note::F4 => 65,
                Note::E4 => 64,
                Note::DS4 => 63,
                Note::D4 => 62,
                Note::CS4 => 61,
                Note::C4 => 60,
                Note::B3 => 59,
                Note::AS3 => 58,
                Note::A3 => 57,
                Note::GS3 => 56,
                Note::G3 => 55,
                Note::FS3 => 54,
                Note::F3 => 53,
                Note::E3 => 52,
                Note::DS3 => 51,
                Note::D3 => 50,
                Note::CS3 => 49,
                Note::C3 => 48,
                Note::B2 => 47,
                Note::AS2 => 46,
                Note::A2 => 45,
                Note::GS2 => 44,
                Note::G2 => 43,
                Note::FS2 => 42,
                Note::F2 => 41,
                Note::E2 => 40,
                Note::DS2 => 39,
                Note::D2 => 38,
                Note::CS2 => 37,
                Note::C2 => 36,
                Note::B1 => 35,
                Note::AS1 => 34,
                Note::A1 => 33,
                Note::GS1 => 32,
                Note::G1 => 31,
                Note::FS1 => 30,
                Note::F1 => 29,
                Note::E1 => 28,
                Note::DS1 => 27,
                Note::D1 => 26,
                Note::CS1 => 25,
                Note::C1 => 24,
                Note::B0 => 23,
                Note::AS0 => 22,
                Note::A0 => 21,
                Note::_20 => 20,
                Note::_19 => 19,
                Note::_18 => 18,
                Note::_17 => 17,
                Note::_16 => 16,
                Note::_15 => 15,
                Note::_14 => 14,
                Note::_13 => 13,
                Note::_12 => 12,
                Note::_11 => 11,
                Note::_10 => 10,
                Note::_9 => 9,
                Note::_8 => 8,
                Note::_7 => 7,
                Note::_6 => 6,
                Note::_5 => 5,
                Note::_4 => 4,
                Note::_3 => 3,
                Note::_2 => 2,
                Note::_1 => 1,
                Note::_0 => 0,
                Note::AcousticBassDrum => 35,
                Note::RideCymbal1 => 51,
                Note::HighAgogo => 67,
                Note::BassDrum1 => 36,
                Note::ChineseCymbal => 52,
                Note::LowAgogo => 68,
                Note::SideStick => 37,
                Note::RideBell => 53,
                Note::Cabasa => 69,
                Note::AcousticSnare => 38,
                Note::Tambourine => 54,
                Note::Maracas => 70,
                Note::HandClap => 39,
                Note::SplashCymbal => 55,
                Note::ShortWhistle => 71,
                Note::ElectricSnare => 40,
                Note::Cowbell => 56,
                Note::LongWhistle => 72,
                Note::LowFloorTom => 41,
                Note::CrashCymbal2 => 57,
                Note::ShortGuiro => 73,
                Note::ClosedHiHat => 42,
                Note::Vibraslap => 58,
                Note::LongGuiro => 74,
                Note::HighFloorTom => 43,
                Note::RideCymbal2 => 59,
                Note::Claves => 75,
                Note::PedalHiHat => 44,
                Note::HiBongo => 60,
                Note::HiWoodBlock => 76,
                Note::LowTom => 45,
                Note::LowBongo => 61,
                Note::LowWoodBlock => 77,
                Note::OpenHiHat => 46,
                Note::MuteHiConga => 62,
                Note::MuteCuica => 78,
                Note::LowMidTom => 47,
                Note::OpenHiConga => 63,
                Note::OpenCuica => 79,
                Note::HiMidTom => 48,
                Note::LowConga => 64,
                Note::MuteTriangle => 80,
                Note::CrashCymbal1 => 49,
                Note::HighTimbale => 65,
                Note::OpenTriangle => 81,
                Note::HighTom => 50,
                Note::LowTimbale => 66,
                Note::Freq(freq) => freq_to_midi_note_cents(*freq).0,
            }
        }

            /// From midi note
        pub fn from_note(note: u8) -> Note {
            match note {
                127 => Note::G9,
                126 => Note::FS9,
                125 => Note::F9,
                124 => Note::E9,
                123 => Note::DS9,
                122 => Note::D9,
                121 => Note::CS9,
                120 => Note::C9,
                119 => Note::B8,
                118 => Note::AS8,
                117 => Note::A8,
                116 => Note::GS8,
                115 => Note::G8,
                114 => Note::FS8,
                113 => Note::F8,
                112 => Note::E8,
                111 => Note::DS8,
                110 => Note::D8,
                109 => Note::CS8,
                108 => Note::C8,
                107 => Note::B7,
                106 => Note::AS7,
                105 => Note::A7,
                104 => Note::GS7,
                103 => Note::G7,
                102 => Note::FS7,
                101 => Note::F7,
                100 => Note::E7,
                99 => Note::DS7,
                98 => Note::D7,
                97 => Note::CS7,
                96 => Note::C7,
                95 => Note::B6,
                94 => Note::AS6,
                93 => Note::A6,
                92 => Note::GS6,
                91 => Note::G6,
                90 => Note::FS6,
                89 => Note::F6,
                88 => Note::E6,
                87 => Note::DS6,
                86 => Note::D6,
                85 => Note::CS6,
                84 => Note::C6,
                83 => Note::B5,
                82 => Note::AS5,
                81 => Note::A5,
                80 => Note::GS5,
                79 => Note::G5,
                78 => Note::FS5,
                77 => Note::F5,
                76 => Note::E5,
                75 => Note::DS5,
                74 => Note::D5,
                73 => Note::CS5,
                72 => Note::C5,
                71 => Note::B4,
                70 => Note::AS4,
                69 => Note::A4,
                68 => Note::GS4,
                67 => Note::G4,
                66 => Note::FS4,
                65 => Note::F4,
                64 => Note::E4,
                63 => Note::DS4,
                62 => Note::D4,
                61 => Note::CS4,
                60 => Note::C4,
                59 => Note::B3,
                58 => Note::AS3,
                57 => Note::A3,
                56 => Note::GS3,
                55 => Note::G3,
                54 => Note::FS3,
                53 => Note::F3,
                52 => Note::E3,
                51 => Note::DS3,
                50 => Note::D3,
                49 => Note::CS3,
                48 => Note::C3,
                47 => Note::B2,
                46 => Note::AS2,
                45 => Note::A2,
                44 => Note::GS2,
                43 => Note::G2,
                42 => Note::FS2,
                41 => Note::F2,
                40 => Note::E2,
                39 => Note::DS2,
                38 => Note::D2,
                37 => Note::CS2,
                36 => Note::C2,
                35 => Note::B1,
                34 => Note::AS1,
                33 => Note::A1,
                32 => Note::GS1,
                31 => Note::G1,
                30 => Note::FS1,
                29 => Note::F1,
                28 => Note::E1,
                27 => Note::DS1,
                26 => Note::D1,
                25 => Note::CS1,
                24 => Note::C1,
                23 => Note::B0,
                22 => Note::AS0,
                21 => Note::A0,
                20 => Note::_20,
                19 => Note::_19,
                18 => Note::_18,
                17 => Note::_17,
                16 => Note::_16,
                15 => Note::_15,
                14 => Note::_14,
                13 => Note::_13,
                12 => Note::_12,
                11 => Note::_11,
                10 => Note::_10,
                9 => Note::_9,
                8 => Note::_8,
                7 => Note::_7,
                6 => Note::_6,
                5 => Note::_5,
                4 => Note::_4,
                3 => Note::_3,
                2 => Note::_2,
                1 => Note::_1,
                0 => Note::_0,
                _ => Note::_0,
            }
        }
    }
}
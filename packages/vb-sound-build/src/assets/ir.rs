use std::collections::BTreeMap;

mod state;
pub use state::decode;

use crate::config::ChannelEffects;

#[derive(Debug)]
pub struct IrInfo {
    pub name: String,
    pub pattern_length: u16,
    pub ticks_per_second: f32,
    pub ticks_per_row: u8,
    pub virtual_tempo_numerator: u16,
    pub virtual_tempo_denominator: u16,
    pub instruments: Vec<Instrument>,
    pub channels: BTreeMap<u8, Channel>,
}

#[derive(Debug)]
pub struct Channel {
    pub patterns: BTreeMap<u8, Pattern>,
    pub order: Vec<u8>,
    pub effects: ChannelEffects,
}

#[derive(Debug)]
pub struct Pattern {
    pub data: BTreeMap<u8, PatternRow>,
}

#[derive(Debug, Clone)]
pub struct PatternRow {
    pub note: Option<NoteEvent>,
    pub instrument: Option<usize>,
    pub volume: Option<f64>,
    pub effects: Vec<Effect>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum NoteEvent {
    Start(u8),
    Stop,
    Release,
}

#[derive(Debug, Clone)]
pub enum Effect {
    Pitch(PitchEffect),
    Volume(VolumeEffect),
    Panning(PanningEffect),
    Control(ControlEffect),
}

#[derive(Debug, Clone)]
pub enum PitchEffect {
    Arpeggio(u8, u8),
    PitchSlide(i16),
    Portamento(i16),
    Vibrato(u8, u8),
    ArpeggioSpeed(u8),
    NoteCut(u8),
    NoteRelease(u8),
}

#[derive(Debug, Clone)]
pub enum VolumeEffect {
    VolumeSlide(f64),
}

#[allow(clippy::enum_variant_names)]
#[derive(Debug, Clone)]
pub enum PanningEffect {
    SetPanning(f64, f64),
    SetVolumeLeft(f64),
    SetVolumeRight(f64),
}

#[derive(Debug, Clone)]
pub enum ControlEffect {
    JumpToOrder(u8),
    JumpToNextPattern(u8),
    SetVirtualTempoNumerator(u8),
    SetVirtualTempoDenominator(u8),
    StopSong,
}

#[derive(Debug)]
pub struct Instrument {
    pub waveform: Option<[u8; 32]>,
    pub volume_macro: Option<InstrumentMacro<f64>>,
    pub arpeggio_macro: Option<InstrumentMacro<i8>>,
    pub waveform_macro: Option<InstrumentMacro<[u8; 32]>>,
}

#[derive(Debug)]
pub struct InstrumentMacro<T> {
    pub macro_loop: i8,
    pub macro_release: i8,
    pub macro_delay: u8,
    pub macro_speed: u8,
    pub data: Vec<T>,
}

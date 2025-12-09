use std::{
    collections::{BTreeMap, HashMap},
    ops::{Add, Sub},
    time::Duration,
};

use crate::{assets::Channel, config::ChannelEffects};

#[derive(Debug, Default)]
struct SoundRow {
    note: Option<NoteEvent>,
    frequency: Option<u16>,
    waveform: Option<u8>,
    volume: Option<u8>,
    envelope: Option<u8>,
    tap: Option<u8>,
    start_pattern: Option<u8>,
    go_to_pattern: Option<u8>,
}

impl SoundRow {
    fn merge(self, other: Self) -> Self {
        Self {
            waveform: other.waveform.or(self.waveform),
            frequency: other.frequency.or(self.frequency),
            volume: other.volume.or(self.volume),
            envelope: other.envelope.or(self.envelope),
            note: other.note.or(self.note),
            tap: other.tap.or(self.tap),
            start_pattern: other.start_pattern.or(self.start_pattern),
            go_to_pattern: other.go_to_pattern.or(self.go_to_pattern),
        }
    }
}

#[derive(Debug)]
enum NoteEvent {
    Start(NoteStart),
    Stop,
}

#[derive(Debug)]
struct NoteStart {
    interval: Option<u8>,
}

// the longest time which the VB's automatic shutoff interval can process
const INTERVAL_UNIT: Duration = Duration::from_nanos(3_840_246);

pub struct ChannelPlayer {
    effects: ChannelEffects,
    noise: bool,
    use_interval: bool,
    init: SoundRow,
    timeline: BTreeMap<Moment, SoundRow>,
    now: Option<Moment>,
    note_started: Option<Moment>,
    last_key: Option<u8>,
    last_waveform: Option<u8>,
    last_volume: Option<u8>,
    last_envelope: Option<u8>,
    last_tap: Option<u8>,
    pitch_shift: f64,
}

impl ChannelPlayer {
    pub fn new(effects: ChannelEffects, use_interval: bool) -> Self {
        Self {
            effects,
            noise: false,
            use_interval,
            init: SoundRow::default(),
            timeline: BTreeMap::new(),
            now: None,
            note_started: None,
            last_key: None,
            last_waveform: None,
            last_volume: None,
            last_envelope: None,
            last_tap: None,
            pitch_shift: 0.0,
        }
    }

    pub fn advance_time(&mut self, now: Moment) {
        assert!(self.now.is_none_or(|n| n <= now));
        self.now = Some(now);
    }

    pub fn start_pattern(&mut self, index: u8) {
        self.current_row().start_pattern = Some(index);
    }

    pub fn go_to_pattern(&mut self, index: u8) {
        self.current_row().go_to_pattern = Some(index);
    }

    pub fn set_waveform(&mut self, waveform: u8) {
        if self.last_waveform != Some(waveform) {
            self.current_row().waveform = Some(waveform);
            self.last_waveform = Some(waveform);
        }
    }

    pub fn set_volume(&mut self, volume: u8) {
        let volume = (volume as f64 * self.effects.volume) as u8;
        if self.last_volume != Some(volume) {
            self.current_row().volume = Some(volume);
            self.last_volume = Some(volume);
        }
    }

    pub fn set_envelope(&mut self, envelope: u8) {
        if self.last_envelope != Some(envelope) {
            self.current_row().envelope = Some(envelope);
            self.last_envelope = Some(envelope);
        }
    }

    pub fn set_tap(&mut self, tap: u8) {
        self.noise = true;
        if self.last_tap != Some(tap) {
            self.current_row().tap = Some(tap);
            self.last_tap = Some(tap);
        }
    }

    pub fn start_note(&mut self, mut key: u8) {
        if self.note_started.is_some() {
            self.stop_note();
        }
        if self.noise {
            key = (key as u16 * 3 / 4) as u8;
        }
        if self.last_key != Some(key) {
            let frequency =
                key_to_clocks(key, self.effects.shift + self.pitch_shift).expect("note is too low");
            self.current_row().frequency = Some(frequency);
            self.last_key = Some(key);
        }
        self.current_row().note = Some(NoteEvent::Start(NoteStart { interval: None }));
        self.note_started = self.now;
    }

    pub fn stop_note(&mut self) {
        let Some(started) = self.note_started.take() else {
            panic!("stopped a note which was never started");
        };
        let interval_units = (self.now.unwrap_or(Moment::START) - started)
            .div_duration_f32(INTERVAL_UNIT)
            .round() as u8;
        if self.use_interval && interval_units < 32 {
            let Some(NoteEvent::Start(note)) = self
                .timeline
                .get_mut(&started)
                .and_then(|s| s.note.as_mut())
            else {
                panic!("invalid note_started");
            };
            note.interval = Some(interval_units - 1);
        } else {
            let row = self.current_row();
            assert!(row.note.is_none());
            row.note = Some(NoteEvent::Stop);
        }
    }

    pub fn set_pitch_shift(&mut self, shift: f64) {
        if self.pitch_shift != shift {
            self.pitch_shift = shift;
            if let Some(key) = self.last_key {
                let frequency = key_to_clocks(key, self.effects.shift + self.pitch_shift)
                    .expect("note is too low");
                self.current_row().frequency = Some(frequency);
            }
        }
    }

    pub fn finish(self) -> Vec<VBEvent> {
        let mut events = vec![];
        emit_events(self.init, &mut events);
        let mut current_row = SoundRow::default();
        let mut current_frame = 0;
        for (moment, row) in self.timeline {
            let frame = moment.last_frame();
            if frame == current_frame {
                current_row = current_row.merge(row);
            } else {
                emit_events(current_row, &mut events);
                if current_frame == 0 && !self.noise {
                    events.push(VBEvent::SetEnvelopeMod {
                        enabled: false,
                        repeat: false,
                    });
                }
                let frames = frame - current_frame;
                events.push(VBEvent::Wait { frames });
                current_row = row;
                current_frame = frame;
            }
        }
        emit_events(current_row, &mut events);
        let frames = self
            .now
            .map(|n| n.last_frame() - current_frame)
            .unwrap_or_default();
        if frames > 0 {
            events.push(VBEvent::Wait { frames });
        }
        events.push(VBEvent::Stop);
        events
    }

    fn current_row(&mut self) -> &mut SoundRow {
        match self.now {
            Some(now) => self.timeline.entry(now).or_default(),
            None => &mut self.init,
        }
    }
}

pub struct ChannelBuilder {
    pub name: String,
    pub player: ChannelPlayer,
}
impl ChannelBuilder {
    pub fn build(self) -> Channel {
        let mut encoder = EventEncoder::new();
        for event in self.player.finish() {
            encoder.encode(event);
        }
        Channel {
            name: self.name,
            data: encoder.finish(),
        }
    }
}

fn emit_events(row: SoundRow, events: &mut Vec<VBEvent>) {
    if let Some(index) = row.start_pattern {
        events.push(VBEvent::StartPattern { index });
    }
    if let Some(frequency) = row.frequency {
        events.push(VBEvent::SetFrequency { frequency });
    }
    if let Some(waveform) = row.waveform {
        events.push(VBEvent::SetWaveform { waveform });
    }
    if let Some(volume) = row.volume {
        events.push(VBEvent::SetVolume {
            left: volume,
            right: volume,
        });
    }
    if let Some(envelope) = row.envelope {
        events.push(VBEvent::SetEnvelope {
            value: envelope,
            grow: false,
            interval: 0,
        });
    }
    if let Some(tap) = row.tap {
        events.push(VBEvent::SetTap { tap });
    }
    match row.note {
        Some(NoteEvent::Start(note)) => {
            if let Some(interval) = note.interval {
                events.push(VBEvent::SetInterval {
                    enabled: true,
                    auto: true,
                    interval,
                });
            } else {
                events.push(VBEvent::SetInterval {
                    enabled: true,
                    auto: false,
                    interval: 0,
                });
            }
        }
        Some(NoteEvent::Stop) => {
            events.push(VBEvent::SetInterval {
                enabled: false,
                auto: false,
                interval: 0,
            });
        }
        None => {}
    }
    if let Some(index) = row.go_to_pattern {
        events.push(VBEvent::GoToPattern { index });
    }
}

fn key_to_pitch(key: u8, shift: f64) -> f64 {
    const A4_KEY: u8 = 69;

    let semitones_from_a4 = key as f64 - A4_KEY as f64;
    440.0 * 2.0f64.powf((semitones_from_a4 + shift) / 12.0)
}

fn pitch_to_clocks(pitch: f64) -> Option<u16> {
    let freq = 2048.0 - (156_250.0 / pitch);
    if freq > 0.0 { Some(freq as u16) } else { None }
}

fn key_to_clocks(key: u8, shift: f64) -> Option<u16> {
    pitch_to_clocks(key_to_pitch(key, shift))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Moment(Duration);
impl Moment {
    pub const START: Self = Self(Duration::ZERO);
    fn last_frame(self) -> u32 {
        (self.0.as_millis() / 20) as u32
    }
}

impl Add<Duration> for Moment {
    type Output = Self;
    fn add(self, rhs: Duration) -> Self::Output {
        Self(self.0 + rhs)
    }
}

impl Sub for Moment {
    type Output = Duration;

    fn sub(self, rhs: Self) -> Self::Output {
        self.0 - rhs.0
    }
}

#[derive(Debug)]
pub enum VBEvent {
    Wait {
        frames: u32,
    },
    SetWaveform {
        waveform: u8,
    },
    SetVolume {
        left: u8,
        right: u8,
    },
    SetEnvelope {
        value: u8,
        grow: bool,
        interval: u8,
    },
    SetEnvelopeMod {
        enabled: bool,
        repeat: bool,
    },
    SetTap {
        tap: u8,
    },
    SetFrequency {
        frequency: u16,
    },
    SetInterval {
        enabled: bool,
        auto: bool,
        interval: u8,
    },
    StartPattern {
        index: u8,
    },
    GoToPattern {
        index: u8,
    },
    Stop,
}

struct EventEncoder {
    bytes: Vec<u8>,
    patterns: HashMap<u8, i32>,
}
impl EventEncoder {
    const WAIT: u8 = 0;
    const WRITE: u8 = 1;
    const JUMP: u8 = 2;

    pub fn new() -> Self {
        Self {
            bytes: vec![],
            patterns: HashMap::new(),
        }
    }

    pub fn encode(&mut self, event: VBEvent) {
        match event {
            VBEvent::Wait { frames } => {
                assert_ne!(frames, 0);
                self.bytes.push(Self::WAIT);
                self.encode_u24(frames);
            }
            VBEvent::SetWaveform { waveform } => {
                self.encode_write(0x18, waveform);
            }
            VBEvent::SetVolume { left, right } => {
                let lrv = ((left << 4) & 0xf0) | (right & 0x0f);
                self.encode_write(0x04, lrv);
            }
            VBEvent::SetEnvelope {
                value,
                grow,
                interval,
            } => {
                let ev0 = (value << 4) | (if grow { 0x08 } else { 0x00 }) | (interval & 0x07);
                self.encode_write(0x10, ev0);
            }
            VBEvent::SetEnvelopeMod { enabled, repeat } => {
                let ev1 = (if repeat { 0x02 } else { 0x00 }) | (if enabled { 0x01 } else { 0x00 });
                self.encode_write(0x14, ev1);
            }
            VBEvent::SetTap { tap } => {
                let ev1 = tap << 4;
                self.encode_write(0x14, ev1);
            }
            VBEvent::SetFrequency { frequency } => {
                let lo = (frequency & 0xff) as u8;
                self.encode_write(0x08, lo);
                let hi = (frequency >> 8 & 0x07) as u8;
                self.encode_write(0x0c, hi);
            }
            VBEvent::SetInterval {
                enabled,
                auto,
                interval,
            } => {
                let int = (if enabled { 0x80 } else { 0x00 })
                    | (if auto { 0x02 } else { 0x00 })
                    | (interval & 0x1f);
                self.encode_write(0x00, int);
            }
            VBEvent::StartPattern { index } => {
                self.patterns.insert(index, self.bytes.len() as i32 / 4);
            }
            VBEvent::GoToPattern { index } => {
                let target = *self.patterns.get(&index).expect("unrecognized pattern");
                let current = self.bytes.len() as i32 / 4;
                self.bytes.push(Self::JUMP);
                self.encode_i24(target - current);
            }
            VBEvent::Stop => {
                for _ in 0..4 {
                    self.bytes.push(0);
                }
            }
        }
    }

    pub fn finish(mut self) -> Vec<u8> {
        for _ in 0..4 {
            self.bytes.push(0);
        }
        self.bytes
    }

    fn encode_u24(&mut self, value: u32) {
        let bytes = value.to_le_bytes();
        assert_eq!(bytes[3], 0);
        for byte in &bytes[0..3] {
            self.bytes.push(*byte);
        }
    }

    fn encode_i24(&mut self, value: i32) {
        let bytes = value.to_le_bytes();
        if value.is_negative() {
            assert_eq!(bytes[3], 255);
        } else {
            assert_eq!(bytes[3], 0);
        }
        for byte in &bytes[0..3] {
            self.bytes.push(*byte);
        }
    }

    fn encode_write(&mut self, offset: u8, value: u8) {
        self.bytes.push(Self::WRITE);
        self.bytes.push(0);
        self.bytes.push(offset);
        self.bytes.push(value);
    }
}

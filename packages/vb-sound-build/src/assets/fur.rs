mod parser;

use anyhow::Result;
use binrw::BinRead;
use flate2::bufread::ZlibDecoder;
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    fs,
    io::{Cursor, Read},
    path::Path,
    time::Duration,
};

use crate::{
    assets::{
        ChannelData, WaveformSetData,
        fur::parser::{
            FurEffect, FurHeader, FurInfoBlock, FurInstrument, FurMacro, FurMacroBody,
            FurPatternRow,
        },
        sound::{ChannelBuilder, ChannelPlayer, Moment},
    },
    config::ChannelEffects,
};

pub struct FurDecoder {
    name: String,
    info: FurInfoBlock,
    looping: bool,
}
impl FurDecoder {
    pub fn new(name: &str, file: &Path, looping: bool) -> Result<Self> {
        let raw_bytes = fs::read(file)?;
        let mut decoder = ZlibDecoder::new(raw_bytes.as_slice());
        let mut bytes = vec![];
        decoder.read_to_end(&mut bytes)?;
        let header = FurHeader::read(&mut Cursor::new(bytes))?;
        Ok(Self {
            name: name.to_string(),
            info: header.pointer.value,
            looping,
        })
    }

    pub fn wavetable(&self, index: usize) -> Option<[u8; 32]> {
        let wavetable = &self.info.wavetables.get(index)?.value;
        assert!(wavetable.data.len() == 32, "Invalid wavetable data");
        let mut result = [0; 32];
        for (index, sample) in wavetable.data.iter().enumerate() {
            result[index] = *sample as u8;
        }
        Some(result)
    }

    pub fn decode(self, waveforms: &mut WaveformSetData) -> Result<Vec<ChannelData>> {
        let info = &self.info;
        assert!(info.subsong_pointers.is_empty());
        assert!(info.samples.is_empty());
        let mut channels: Vec<FurChannel> = (0..6).map(|i| FurChannel::new(info, i)).collect();
        let mut orders_seen = HashSet::new();
        let mut order = 0;
        let mut start_row = 0;
        loop {
            if !orders_seen.insert(order) {
                for channel in &mut channels {
                    channel.jump_to(order);
                }
                break;
            }
            let mut end = None;
            for channel in &channels {
                let pattern_length = channel.pattern_length(order);
                end = match (pattern_length, end) {
                    (Some(old), Some(new)) => Some(old.min(new)),
                    (a, b) => a.or(b),
                };
            }
            let end_row = end.unwrap_or(info.pattern_length as u64);
            let mut next = NextPosition::NextPattern;
            for channel in &mut channels {
                let pos = channel.play_pattern(order, start_row, end_row, info, waveforms)?;
                next = next.max(pos);
            }
            if let NextPosition::NextPattern = &next
                && order == info.orders_length as usize - 1
            {
                next = if self.looping {
                    NextPosition::NextPattern
                } else {
                    NextPosition::Stop
                }
            }
            (order, start_row) = match next {
                NextPosition::NextPattern => (order + 1, 0),
                NextPosition::Pattern { order, row } => (order, row),
                NextPosition::Stop => break,
            }
        }
        Ok(channels
            .into_iter()
            .flat_map(|c| c.build(&self.name))
            .collect())
    }
}

struct FurChannel {
    channel: usize,
    player: ChannelPlayer,
    clock: Clock,
    orders: Vec<u16>,
    played_orders: HashSet<usize>,
    patterns: HashMap<u16, Vec<FurPatternRow>>,
    effects: EffectCursor,
    empty: bool,
}

impl FurChannel {
    fn new(info: &FurInfoBlock, channel: usize) -> Self {
        let mut player = ChannelPlayer::new(ChannelEffects::default(), false);
        let mut patterns = HashMap::new();
        for pattern in &info.patterns {
            let value = &pattern.value;
            if value.channel == channel as u8 {
                patterns.insert(value.index, value.data.to_vec());
            }
        }
        player.set_volume(15);
        player.set_envelope(15);
        Self {
            channel,
            player,
            clock: Clock::new(info),
            orders: info.orders[channel].iter().map(|x| *x as u16).collect(),
            played_orders: HashSet::new(),
            patterns,
            effects: EffectCursor::new(),
            empty: true,
        }
    }

    fn pattern_length(&self, order: usize) -> Option<u64> {
        let pattern_index = self.orders.get(order)?;
        let pattern = self.patterns.get(pattern_index)?;
        for p in pattern {
            for effect in &p.effects {
                match effect {
                    FurEffect::JumpToOrder(_) | FurEffect::JumpToNextPattern(_) => {
                        return Some(p.index + 1);
                    }
                    FurEffect::StopSong => return Some(p.index),
                    _ => {}
                }
            }
        }
        None
    }

    fn jump_to(&mut self, order: usize) {
        self.player.go_to_pattern(order as u8);
    }

    fn play_pattern(
        &mut self,
        order: usize,
        start_row: u64,
        end_row: u64,
        info: &FurInfoBlock,
        waveforms: &mut WaveformSetData,
    ) -> Result<NextPosition> {
        self.player.advance_time(self.clock.now());
        self.player.start_pattern(order as u8);
        self.played_orders.insert(order);
        let ticks_per_row = info.speed_1 as u64;
        let start_tick = self.clock.now_tick();
        let end_tick = start_tick + ((end_row - start_row) * ticks_per_row);
        let mut next = NextPosition::NextPattern;
        if let Some(pattern_index) = self.orders.get(order)
            && let Some(pattern) = self.patterns.get(pattern_index)
        {
            let rows: Vec<FurPatternRow> = pattern
                .iter()
                .skip_while(|r| r.index < start_row)
                .take_while(|r| r.index < end_row)
                .cloned()
                .collect();
            'pattern_loop: for row in rows {
                let tick = start_tick + row.index * ticks_per_row;
                self.advance_to(tick);

                if let Some(volume) = row.volume {
                    self.player.set_volume(volume);
                }
                if let Some(instrument) = row.instrument {
                    let instr = &info.instruments[instrument as usize].value;
                    let wavedata_index = if let Some(synth) = instr.wavetable_synth_data() {
                        synth.first_wave as usize
                    } else {
                        0
                    };
                    let wavedata = find_wavetable(info, wavedata_index).expect("Invalid wavetable");
                    let index = waveforms.add_waveform(wavedata)?;
                    self.player.set_waveform(index);
                    self.effects.load_instrument(instr, self.clock.now_tick());
                }
                self.effects
                    .load_effects(info, &row.effects, self.clock.now_tick());
                for effect in self.effects.effects(self.clock.now_tick()) {
                    effect.apply(&mut self.player);
                }
                if let Some(note) = row.note {
                    if note == 180 {
                        if let Some(key) = self.player.current_note() {
                            self.player.start_note(key);
                        }
                    } else if note > 180 {
                        self.player.stop_note();
                    } else {
                        self.player.start_note(note - 48);
                    }
                    self.empty = false;
                }

                for effect in &row.effects {
                    match effect {
                        FurEffect::JumpToNextPattern(row) => {
                            next = NextPosition::Pattern {
                                order: order + 1,
                                row: *row as u64,
                            };
                            break 'pattern_loop;
                        }
                        FurEffect::JumpToOrder(order) => {
                            next = NextPosition::Pattern {
                                order: *order as usize,
                                row: 0,
                            };
                            break 'pattern_loop;
                        }
                        FurEffect::StopSong => {
                            next = NextPosition::Stop;
                            break 'pattern_loop;
                        }
                        _ => {}
                    }
                }
            }
        }
        self.advance_to(end_tick);
        Ok(next)
    }

    fn advance_to(&mut self, tick: u64) {
        for effect in self.effects.effects(tick) {
            self.clock.advance(effect.tick);
            self.player.advance_time(self.clock.now());
            effect.apply(&mut self.player);
        }
        self.clock.advance(tick);
        self.player.advance_time(self.clock.now());
    }

    fn build(self, name: &str) -> Option<ChannelData> {
        if self.empty {
            return None;
        }
        let builder = ChannelBuilder {
            name: format!("{name}_{}", self.channel),
            player: self.player,
        };
        Some(builder.build())
    }
}

#[derive(PartialEq, Eq, PartialOrd, Ord)]
enum NextPosition {
    NextPattern,
    Pattern { order: usize, row: u64 },
    Stop,
}

fn find_wavetable(info: &FurInfoBlock, index: usize) -> Option<[u8; 32]> {
    let wavetable = &info.wavetables.get(index)?.value;
    assert!(wavetable.data.len() == 32, "Invalid wavetable data");
    let mut result = [0; 32];
    for (index, sample) in wavetable.data.iter().enumerate() {
        result[index] = *sample as u8;
    }
    Some(result)
}

struct Clock {
    per_tick: Duration,
    elapsed: Duration,
    tick: u64,
}
impl Clock {
    fn new(info: &FurInfoBlock) -> Self {
        Self {
            per_tick: Duration::from_secs_f32(1.0 / info.ticks_per_second),
            elapsed: Duration::ZERO,
            tick: 0,
        }
    }

    fn advance(&mut self, now_ticks: u64) {
        self.elapsed = self.per_tick * now_ticks as u32;
        self.tick = now_ticks;
    }

    fn now(&self) -> Moment {
        Moment::START + self.elapsed
    }

    fn now_tick(&self) -> u64 {
        self.tick
    }
}

#[derive(Debug)]
struct MacroBodyCursor<T> {
    data: Vec<T>,
    speed: u64,
    loop_to: Option<usize>,
    next_event: Option<(u64, usize)>,
}
impl<T> MacroBodyCursor<T>
where
    T: BinRead + Copy + std::fmt::Debug,
    for<'a> T::Args<'a>: Default + Clone,
{
    fn load(body: &FurMacroBody<T>, at_tick: u64) -> Self {
        let next_event = if body.data.is_empty() {
            None
        } else {
            Some((body.macro_delay as u64 + at_tick, 0))
        };
        Self {
            data: body.data.clone(),
            speed: body.macro_speed as u64,
            loop_to: body
                .macro_loop
                .try_into()
                .ok()
                .filter(|l| *l < body.data.len()),
            next_event,
        }
    }

    fn values(&mut self, until: u64) -> Vec<(u64, T)> {
        let mut result = vec![];
        while let Some((time, value)) = self.next_value(until) {
            result.push((time, value));
        }
        result
    }

    fn next_value(&mut self, until: u64) -> Option<(u64, T)> {
        let (time, idx) = self.next_event.take_if(|(t, _)| *t <= until)?;
        if idx < self.data.len() - 1 {
            self.next_event = Some((time + self.speed, idx + 1));
        } else if let Some(loop_to) = self.loop_to {
            self.next_event = Some((time + self.speed, loop_to.min(self.data.len() - 1)));
        }
        Some((time, self.data[idx]))
    }
}

#[derive(Debug)]
struct ArpeggioEffectCursor {
    tick: u64,
    speed: u8,
    offset: u8,
    x: u8,
    y: u8,
}
impl ArpeggioEffectCursor {
    fn new(tick: u64, speed: u8, x: u8, y: u8) -> Self {
        Self {
            tick,
            speed,
            offset: 0,
            x,
            y,
        }
    }

    fn values(&mut self, until_tick: u64) -> Vec<(u64, i16)> {
        let mut result = vec![];
        while self.tick <= until_tick {
            let value = match self.offset {
                2 => self.y,
                1 => self.x,
                _ => 0,
            };
            result.push((self.tick, value as i16));
            self.tick += self.speed as u64;
            self.offset += 1;
            if self.offset > 2 {
                self.offset = 0
            }
        }
        result
    }
}

#[derive(Debug)]
struct PitchSlideCursor {
    tick: u64,
    speed: i16,
    value: i16,
}

impl PitchSlideCursor {
    fn new(tick: u64, speed: i16) -> Self {
        Self {
            tick,
            speed,
            value: 0,
        }
    }

    fn values(&mut self, until_tick: u64) -> Vec<(u64, i16)> {
        let mut result = vec![];
        while self.tick <= until_tick {
            result.push((self.tick, self.value));
            self.tick += 1;
            self.value += self.speed;
        }
        result
    }
}

#[derive(Debug)]
struct EffectCursor {
    volume: Option<MacroBodyCursor<u8>>,
    arpeggio: Option<MacroBodyCursor<i8>>,
    arpeggio_effect: Option<ArpeggioEffectCursor>,
    arpeggio_speed: u8,
    pitch_slide: Option<PitchSlideCursor>,
    note_release: Option<u64>,
}

impl EffectCursor {
    fn new() -> Self {
        Self {
            volume: None,
            arpeggio: None,
            arpeggio_effect: None,
            arpeggio_speed: 1,
            pitch_slide: None,
            note_release: None,
        }
    }

    fn load_instrument(&mut self, instr: &FurInstrument, at_tick: u64) {
        self.volume = None;
        self.arpeggio = None;
        if let Some(macros) = instr.macros() {
            for m in macros {
                match m {
                    FurMacro::Volume(v) => self.volume = Some(MacroBodyCursor::load(v, at_tick)),
                    FurMacro::Arpeggio(v) => {
                        self.arpeggio = Some(MacroBodyCursor::load(v, at_tick))
                    }
                    _ => {}
                }
            }
        }
    }

    fn load_effects(&mut self, info: &FurInfoBlock, effects: &[FurEffect], at_tick: u64) {
        for &effect in effects {
            match effect {
                FurEffect::Arpeggio(x, y) => {
                    self.load_arpeggio(x, y, at_tick);
                }
                FurEffect::PitchSlideUp(speed) => {
                    self.load_pitch_slide(info, speed as i16, at_tick)
                }
                FurEffect::PitchSlideDown(speed) => {
                    self.load_pitch_slide(info, -(speed as i16), at_tick)
                }
                FurEffect::ArpeggioSpeed(speed) => {
                    self.arpeggio_speed = speed;
                    if let Some(arp) = self.arpeggio_effect.as_mut() {
                        arp.speed = speed;
                    }
                }
                FurEffect::NoteCut(ticks) | FurEffect::NoteRelease(ticks) => {
                    self.note_release = Some(at_tick + ticks as u64);
                }
                FurEffect::Unknown(effect, value) => {
                    panic!("unknown effect: {effect:02x}{value:02x}");
                }
                _ => {}
            }
        }
    }

    fn load_arpeggio(&mut self, x: u8, y: u8, at_tick: u64) {
        self.arpeggio_effect = Some(ArpeggioEffectCursor::new(
            at_tick,
            self.arpeggio_speed,
            x,
            y,
        ));
    }

    fn load_pitch_slide(&mut self, info: &FurInfoBlock, speed: i16, at_tick: u64) {
        assert_eq!(info.linear_pitch, 1);
        let speed = info.pitch_slide_speed as i16 * speed;
        self.pitch_slide = Some(PitchSlideCursor::new(at_tick, speed));
    }

    fn effects(&mut self, until_tick: u64) -> Vec<MacroEffect> {
        let mut effects = BTreeMap::new();
        if let Some(vol) = self.volume.as_mut() {
            for (tick, vol) in vol.values(until_tick) {
                effects.entry(tick).or_insert(MacroEffect::new(tick)).volume = Some(vol);
            }
        }
        if let Some(arp) = self.arpeggio.as_mut() {
            for (tick, arp) in arp.values(until_tick) {
                effects.entry(tick).or_insert(MacroEffect::new(tick)).pitch = Some(arp as f64);
            }
        }
        if let Some(arp) = self.arpeggio_effect.as_mut() {
            for (tick, arp) in arp.values(until_tick) {
                let effect = effects.entry(tick).or_insert(MacroEffect::new(tick));
                effect.pitch = Some(effect.pitch.unwrap_or_default() + arp as f64);
            }
        }
        if let Some(pitch) = self.pitch_slide.as_mut() {
            for (tick, pitch) in pitch.values(until_tick) {
                let effect = effects.entry(tick).or_insert(MacroEffect::new(tick));
                effect.pitch = Some(effect.pitch.unwrap_or_default() + (pitch as f64 / 128.0));
            }
        }
        if let Some(tick) = self.note_release.take_if(|t| *t <= until_tick) {
            let effect = effects.entry(tick).or_insert(MacroEffect::new(tick));
            effect.release = true;
        }
        effects.into_values().collect()
    }
}

#[derive(Debug)]
struct MacroEffect {
    tick: u64,
    volume: Option<u8>,
    pitch: Option<f64>,
    release: bool,
}
impl MacroEffect {
    fn new(tick: u64) -> Self {
        Self {
            tick,
            volume: None,
            pitch: None,
            release: false,
        }
    }
    fn apply(&self, player: &mut ChannelPlayer) {
        if let Some(volume) = self.volume {
            player.set_envelope(volume);
        }
        if let Some(pitch) = self.pitch {
            player.set_pitch_shift(pitch);
        }
        if self.release {
            player.stop_note();
        }
    }
}

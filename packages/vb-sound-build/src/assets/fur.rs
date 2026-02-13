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
        let mut clock = Clock::new(info);
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
                let pattern_length = channel.preprocess_pattern(order, &mut clock, info);
                end = match (pattern_length, end) {
                    (Some(old), Some(new)) => Some(old.min(new)),
                    (a, b) => a.or(b),
                };
            }
            let end_row = end.unwrap_or(info.pattern_length as u64);
            let mut next = NextPosition::NextPattern;
            for channel in &mut channels {
                let pos =
                    channel.play_pattern(order, start_row, end_row, &clock, info, waveforms)?;
                next = next.max(pos);
            }
            if let NextPosition::NextPattern = &next
                && order == info.orders_length as usize - 1
            {
                next = if self.looping {
                    NextPosition::Pattern { order: 0, row: 0 }
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
    tick: u64,
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
        player.set_volume((15, 15));
        player.set_envelope(15);
        Self {
            channel,
            player,
            tick: 0,
            orders: info.orders[channel].iter().map(|x| *x as u16).collect(),
            played_orders: HashSet::new(),
            patterns,
            effects: EffectCursor::new(),
            empty: true,
        }
    }

    // Calculate how many rows we will process in this pattern.
    // Also, track any tempo changes.
    fn preprocess_pattern(
        &self,
        order: usize,
        clock: &mut Clock,
        info: &FurInfoBlock,
    ) -> Option<u64> {
        let pattern_index = self.orders.get(order)?;
        let pattern = self.patterns.get(pattern_index)?;
        for p in pattern {
            for effect in &p.effects {
                match effect {
                    FurEffect::JumpToOrder(_)
                    | FurEffect::JumpToNextPattern(_)
                    | FurEffect::StopSong => return Some(p.index + 1),
                    FurEffect::SetVirtualTempoNumerator(value) => {
                        let tick = self.tick + (p.index * info.speed_1 as u64);
                        clock.set_virtual_numerator(tick, *value as u16);
                    }
                    FurEffect::SetVirtualTempoDenominator(value) => {
                        let tick = self.tick + (p.index * info.speed_1 as u64);
                        clock.set_virtual_denominator(tick, *value as u16);
                    }
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
        clock: &Clock,
        info: &FurInfoBlock,
        waveforms: &mut WaveformSetData,
    ) -> Result<NextPosition> {
        self.player.advance_time(clock.moment(self.tick));
        self.player.start_pattern(order as u8);
        self.played_orders.insert(order);
        let ticks_per_row = info.speed_1 as u64;
        let start_tick = self.tick;
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
                self.advance_to(tick, clock, info, waveforms)?;

                if let Some(volume) = row.volume {
                    self.player.set_envelope(volume);
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
                    self.effects.load_instrument(instr);
                }
                self.effects.load_effects(info, &row.effects);
                self.player.set_volume(self.effects.panning);
                for effect in self.effects.effects(self.tick) {
                    effect.apply(&mut self.player, info, waveforms)?;
                }
                if let Some(note) = row.note {
                    if note == 182 {
                        // macro release
                        self.effects.release_macros();
                    } else if note == 181 {
                        // note release (seems to do the same as macro release)
                        self.effects.release_macros();
                    } else if note == 180 {
                        // note off
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
        self.advance_to(end_tick, clock, info, waveforms)?;
        Ok(next)
    }

    fn advance_to(
        &mut self,
        tick: u64,
        clock: &Clock,
        info: &FurInfoBlock,
        waveforms: &mut WaveformSetData,
    ) -> Result<()> {
        for effect in self.effects.effects(tick) {
            self.player.advance_time(clock.moment(effect.tick));
            effect.apply(&mut self.player, info, waveforms)?;
        }
        self.tick = tick;
        self.player.advance_time(clock.moment(tick));
        Ok(())
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

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
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
    tempos: BTreeMap<u64, Tempo>,
}
impl Clock {
    fn new(info: &FurInfoBlock) -> Self {
        let initial_tempo = Tempo::new(
            Moment::START,
            info.ticks_per_second,
            info.virtual_tempo_numerator,
            info.virtual_tempo_denominator,
        );
        let mut tempos = BTreeMap::new();
        tempos.insert(0, initial_tempo);
        Self { tempos }
    }

    fn moment(&self, tick: u64) -> Moment {
        let (tempo, elapsed) = self.tempo_at(tick);
        tempo.start + (tempo.per_tick * elapsed as u32)
    }

    fn set_virtual_numerator(&mut self, tick: u64, value: u16) {
        let (tempo, elapsed) = self.tempo_at(tick);
        if tempo.virtual_numerator == value {
            return;
        }
        let start = tempo.start + (tempo.per_tick * elapsed as u32);
        let new_tempo = Tempo::new(
            start,
            tempo.ticks_per_second,
            value,
            tempo.virtual_denominator,
        );
        self.tempos.insert(tick, new_tempo);
    }

    fn set_virtual_denominator(&mut self, tick: u64, value: u16) {
        let (tempo, elapsed) = self.tempo_at(tick);
        if tempo.virtual_denominator == value {
            return;
        }
        let start = tempo.start + (tempo.per_tick * elapsed as u32);
        let new_tempo = Tempo::new(
            start,
            tempo.ticks_per_second,
            tempo.virtual_numerator,
            value,
        );
        self.tempos.insert(tick, new_tempo);
    }

    fn tempo_at(&self, tick: u64) -> (&Tempo, u64) {
        let (start_tick, tempo) = self.tempos.range(..=tick).next_back().unwrap();
        let elapsed = tick - *start_tick;
        (tempo, elapsed)
    }
}

struct Tempo {
    start: Moment,
    ticks_per_second: f32,
    virtual_numerator: u16,
    virtual_denominator: u16,
    per_tick: Duration,
}
impl Tempo {
    fn new(
        start: Moment,
        ticks_per_second: f32,
        virtual_numerator: u16,
        virtual_denominator: u16,
    ) -> Self {
        Self {
            start,
            ticks_per_second,
            virtual_numerator,
            virtual_denominator,
            per_tick: Duration::from_secs_f32(
                virtual_denominator as f32 / (virtual_numerator as f32 * ticks_per_second),
            ),
        }
    }
}

#[derive(Debug)]
struct MacroCursor<T> {
    data: Vec<T>,
    index: usize,
    delay: u64,
    speed: u64,
    release: Option<usize>,
    loop_to: Option<usize>,
}
impl<T> MacroCursor<T>
where
    T: BinRead + Copy + std::fmt::Debug,
    for<'a> T::Args<'a>: Default + Clone,
{
    fn load(body: &FurMacroBody<T>) -> Self {
        let speed = body.macro_speed as u64 - 1;
        Self {
            data: body.data.clone(),
            index: 0,
            delay: body.macro_delay as u64 + speed,
            speed,
            loop_to: body
                .macro_loop
                .try_into()
                .ok()
                .filter(|l| *l < body.data.len()),
            release: body
                .macro_release
                .try_into()
                .ok()
                .filter(|r| *r < body.data.len()),
        }
    }

    fn release(&mut self) {
        self.release = None;
    }
}

impl<T> Iterator for MacroCursor<T>
where
    T: Copy,
{
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        let value = *self.data.get(self.index)?;
        if self.delay == 0 {
            self.delay = self.speed;
            if self.release != Some(self.index) {
                if self.index + 1 < self.data.len() {
                    self.index += 1;
                } else if let Some(to) = self.loop_to {
                    self.index = to;
                }
            }
        } else {
            self.delay -= 1;
        }
        Some(value)
    }
}

#[derive(Debug)]
struct ArpeggioEffectCursor {
    index: u8,
    delay: u8,
    speed: u8,
    x: u8,
    y: u8,
}
impl ArpeggioEffectCursor {
    fn new(speed: u8, x: u8, y: u8) -> Self {
        Self {
            index: 0,
            delay: speed - 1,
            speed: speed - 1,
            x,
            y,
        }
    }
}
impl Iterator for ArpeggioEffectCursor {
    type Item = f64;

    fn next(&mut self) -> Option<Self::Item> {
        let value = match self.index {
            2 => self.y,
            1 => self.x,
            _ => 0,
        };
        if self.delay == 0 {
            self.delay = self.speed;
            self.index += 1;
            if self.index > 2 {
                self.index = 0;
            }
        } else {
            self.delay -= 1;
        }
        Some(value as f64)
    }
}

#[derive(Debug)]
struct VibratoEffectCursor {
    index: u8,
    speed: u8,
    depth: u8,
}
impl VibratoEffectCursor {
    fn new(speed: u8, depth: u8) -> Self {
        Self {
            index: 0,
            speed,
            depth,
        }
    }
}
impl Iterator for VibratoEffectCursor {
    type Item = f64;

    fn next(&mut self) -> Option<Self::Item> {
        // The vibrato pitch shift is controlled by a sine wave,
        // with period of 64/speed steps and amplitude depth/16 semitones.
        let t = self.index as f64 * std::f64::consts::TAU / 64.0;
        let value = t.sin() * self.depth as f64 / 16.0;
        self.index += self.speed;
        while self.index > 64 {
            self.index -= 64;
        }
        Some(value)
    }
}

#[derive(Debug)]
struct SlideCursor {
    speed: i16,
    value: i16,
}
impl SlideCursor {
    fn new(speed: i16) -> Self {
        Self { speed, value: 0 }
    }
}
impl Iterator for SlideCursor {
    type Item = i16;

    fn next(&mut self) -> Option<Self::Item> {
        let value = self.value;
        self.value += self.speed;
        Some(value)
    }
}

#[derive(Debug, Clone)]
struct EffectEntry {
    tick: u64,
    volume: f64,
    pitch: f64,
    waveform: Option<u8>,
    release: bool,
}
impl EffectEntry {
    fn apply(
        &self,
        player: &mut ChannelPlayer,
        info: &FurInfoBlock,
        waveforms: &mut WaveformSetData,
    ) -> Result<()> {
        player.set_envelope_multiplier(self.volume);
        player.set_pitch_shift(self.pitch);
        if self.release {
            player.stop_note();
        }
        if let Some(wavedata_index) = self.waveform {
            let wavedata =
                find_wavetable(info, wavedata_index as usize).expect("Invalid wavetable");
            let index = waveforms.add_waveform(wavedata)?;
            player.set_waveform(index);
        }
        Ok(())
    }
}

#[derive(Debug)]
struct EffectCursor {
    tick: u64,
    instrument_volume: Option<MacroCursor<u8>>,
    instrument_arpeggio: Option<MacroCursor<i8>>,
    instrument_waveform: Option<MacroCursor<u8>>,
    arpeggio_effect: Option<ArpeggioEffectCursor>,
    arpeggio_speed: u8,
    vibrato_effect: Option<VibratoEffectCursor>,
    panning: (u8, u8),
    volume_slide: Option<SlideCursor>,
    pitch_slide: Option<SlideCursor>,
    note_release: Option<u64>,
}

impl EffectCursor {
    fn new() -> Self {
        Self {
            tick: 0,
            instrument_volume: None,
            instrument_arpeggio: None,
            instrument_waveform: None,
            arpeggio_effect: None,
            arpeggio_speed: 1,
            vibrato_effect: None,
            panning: (15, 15),
            volume_slide: None,
            pitch_slide: None,
            note_release: None,
        }
    }

    fn load_instrument(&mut self, instr: &FurInstrument) {
        self.instrument_volume = None;
        self.instrument_arpeggio = None;
        self.instrument_waveform = None;
        if let Some(macros) = instr.macros() {
            for m in macros {
                match m {
                    FurMacro::Volume(v) => self.instrument_volume = Some(MacroCursor::load(v)),
                    FurMacro::Arpeggio(v) => self.instrument_arpeggio = Some(MacroCursor::load(v)),
                    FurMacro::Waveform(v) => self.instrument_waveform = Some(MacroCursor::load(v)),
                    _ => {}
                }
            }
        }
    }

    fn load_effects(&mut self, info: &FurInfoBlock, effects: &[FurEffect]) {
        self.tick = self.tick.saturating_sub(1);
        for &effect in effects {
            match effect {
                FurEffect::Arpeggio(x, y) => {
                    self.load_arpeggio(x, y);
                }
                FurEffect::PitchSlideUp(speed) => self.load_pitch_slide(info, speed as i16),
                FurEffect::PitchSlideDown(speed) => self.load_pitch_slide(info, -(speed as i16)),
                FurEffect::Vibrato(speed, depth) => {
                    self.load_vibrato(speed, depth);
                }
                FurEffect::SetPanning(left, right) => self.panning = (left, right),
                FurEffect::VolumeSlide(up, down) => {
                    let speed = up as i16 - down as i16;
                    self.load_volume_slide(speed);
                }
                FurEffect::SetVolumeLeft(value) => {
                    let vol = (value as f64 * 15.0 / 225.0) as u8;
                    self.panning.0 = vol;
                }
                FurEffect::SetVolumeRight(value) => {
                    let vol = (value as f64 * 15.0 / 225.0) as u8;
                    self.panning.1 = vol;
                }
                FurEffect::ArpeggioSpeed(speed) => {
                    self.arpeggio_speed = speed;
                    if let Some(arp) = self.arpeggio_effect.as_mut() {
                        arp.speed = speed;
                    }
                }
                FurEffect::NoteCut(ticks) | FurEffect::NoteRelease(ticks) => {
                    self.note_release = Some(self.tick + ticks as u64);
                }
                FurEffect::Unknown(effect, value) => {
                    panic!("unknown effect: {effect:02x}{value:02x}");
                }
                _ => {}
            }
        }
    }

    fn load_arpeggio(&mut self, x: u8, y: u8) {
        if x == 0 && y == 0 {
            self.arpeggio_effect = None;
        } else {
            self.arpeggio_effect = Some(ArpeggioEffectCursor::new(self.arpeggio_speed, x, y));
        }
    }

    fn load_vibrato(&mut self, speed: u8, depth: u8) {
        if speed == 0 {
            self.vibrato_effect = None;
        } else {
            self.vibrato_effect = Some(VibratoEffectCursor::new(speed, depth));
        }
    }

    fn load_pitch_slide(&mut self, info: &FurInfoBlock, speed: i16) {
        assert_eq!(info.linear_pitch, 1);
        let speed = info.pitch_slide_speed as i16 * speed;
        self.pitch_slide = Some(SlideCursor::new(speed));
    }

    fn load_volume_slide(&mut self, speed: i16) {
        if speed == 0 {
            self.volume_slide = None;
        } else {
            self.volume_slide = Some(SlideCursor::new(speed));
        }
    }

    fn release_macros(&mut self) {
        if let Some(vol) = &mut self.instrument_volume {
            vol.release();
        }
        if let Some(arp) = &mut self.instrument_arpeggio {
            arp.release();
        }
        if let Some(wav) = &mut self.instrument_waveform {
            wav.release();
        }
    }

    fn effects(&mut self, until_tick: u64) -> Vec<EffectEntry> {
        let mut result = vec![];
        while self.tick <= until_tick {
            let mut volume = 1.0;
            if let Some(vol) = self.instrument_volume.as_mut().and_then(|m| m.next()) {
                volume = vol as f64 / 15.0;
            }
            if let Some(vol) = self.volume_slide.as_mut().and_then(|m| m.next()) {
                let vol = vol.clamp(0, 63) as f64 / 63.0;
                volume = vol;
            }

            let mut pitch = 0.0;
            if let Some(arp) = self.instrument_arpeggio.as_mut().and_then(|m| m.next()) {
                pitch += arp as f64;
            }
            if let Some(arp) = self.arpeggio_effect.as_mut().and_then(|m| m.next()) {
                pitch += arp;
            }
            if let Some(vib) = self.vibrato_effect.as_mut().and_then(|m| m.next()) {
                pitch += vib;
            }
            if let Some(p) = self.pitch_slide.as_mut().and_then(|m| m.next()) {
                pitch += p as f64 / 128.0;
            }

            let waveform = self.instrument_waveform.as_mut().and_then(|m| m.next());

            let release = self.note_release.take_if(|t| *t == self.tick).is_some();

            result.push(EffectEntry {
                tick: self.tick,
                volume,
                pitch,
                waveform,
                release,
            });
            self.tick += 1;
        }
        result
    }
}

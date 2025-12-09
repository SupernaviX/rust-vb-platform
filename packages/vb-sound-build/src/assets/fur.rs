mod parser;

use anyhow::Result;
use binrw::BinRead;
use flate2::bufread::ZlibDecoder;
use std::{
    collections::{BTreeMap, HashMap},
    fs,
    io::{Cursor, Read},
    path::Path,
    time::Duration,
};

use crate::{
    assets::{
        Channel,
        fur::parser::{FurEffect, FurHeader, FurInfoBlock, FurInstrument, FurMacro, FurMacroBody},
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

    pub fn decode(self, waveform_indices: &HashMap<[u8; 32], u8>) -> Result<Vec<Channel>> {
        let info = &self.info;
        assert!(info.subsong_pointers.is_empty());
        assert!(info.samples.is_empty());
        let mut channels = vec![];
        let mut patterns = HashMap::new();
        for pattern in &info.patterns {
            let value = &pattern.value;
            patterns.insert((value.channel, value.index), value.data.as_slice());
        }
        let mut end_tick =
            info.orders_length as u64 * info.pattern_length as u64 * info.speed_1 as u64;
        for channel in 0..6 {
            let mut empty = true;
            let mut player = ChannelPlayer::new(ChannelEffects::default());
            player.set_volume(15);
            player.set_envelope(15);
            let mut clock = Clock::new(info);
            let mut macro_cursor = EffectCursor::new();
            player.advance_time(clock.now());
            if self.looping {
                player.start_pattern(0);
            }
            'play_loop: for (order_index, pattern_index) in info.orders[channel].iter().enumerate()
            {
                let Some(mut pattern) = patterns
                    .get(&(channel as u8, *pattern_index as u16))
                    .copied()
                else {
                    continue;
                };
                let pattern_start_tick =
                    order_index as u64 * info.pattern_length as u64 * info.speed_1 as u64;
                clock.advance(pattern_start_tick);
                while let Some((row, rest)) = pattern.split_first() {
                    pattern = rest;
                    let target_tick = pattern_start_tick + (row.index * info.speed_1 as u64);
                    for effect in macro_cursor.effects(target_tick) {
                        clock.advance(effect.tick);
                        player.advance_time(clock.now());
                        effect.apply(&mut player);
                    }
                    clock.advance(target_tick);
                    player.advance_time(clock.now());

                    if let Some(volume) = row.volume {
                        player.set_volume(volume);
                    }
                    if let Some(instrument) = row.instrument {
                        let instr = &info.instruments[instrument as usize].value;
                        let wavedata_index = if let Some(synth) = instr.wavetable_synth_data() {
                            synth.first_wave as usize
                        } else {
                            0
                        };
                        let wavedata = self.wavetable(wavedata_index).expect("Invalid wavetable");
                        let index = waveform_indices
                            .get(&wavedata)
                            .expect("Unregistered wavedata");
                        player.set_waveform(*index);
                        macro_cursor.load_instrument(instr, clock.now_tick());
                    }
                    macro_cursor.load_effects(info, &row.effects, clock.now_tick());
                    for effect in macro_cursor.effects(clock.now_tick()) {
                        effect.apply(&mut player);
                    }
                    if let Some(note) = row.note {
                        player.start_note(note - 48);
                        empty = false;
                    }

                    if row.should_stop_song() {
                        let target_tick =
                            pattern_start_tick + ((row.index + 1) * info.speed_1 as u64);
                        for effect in macro_cursor.effects(target_tick) {
                            clock.advance(effect.tick);
                            player.advance_time(clock.now());
                            effect.apply(&mut player);
                        }
                        clock.advance(target_tick);
                        end_tick = clock.now_tick();
                        break 'play_loop;
                    }
                }
            }
            for effect in macro_cursor.effects(end_tick) {
                clock.advance(effect.tick);
                player.advance_time(clock.now());
                effect.apply(&mut player);
            }
            clock.advance(end_tick);
            player.advance_time(clock.now());
            if self.looping {
                player.go_to_pattern(0);
            }
            if !empty {
                let builder = ChannelBuilder {
                    name: format!("{}_{channel}", self.name),
                    player,
                };
                channels.push(builder.build());
            }
        }
        Ok(channels)
    }
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

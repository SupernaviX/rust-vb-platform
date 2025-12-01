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
        fur::parser::{FurHeader, FurInfoBlock, FurInstrument, FurMacro, FurMacroBody},
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
        let end_tick = info.orders.len() as u64 * info.pattern_length as u64 * info.speed_1 as u64;
        for channel in 0..6 {
            let mut empty = true;
            let mut player = ChannelPlayer::new(ChannelEffects::default());
            let mut clock = Clock::new(info);
            let mut macro_cursor = MacroCursor::new();
            if self.looping {
                player.start_pattern(0);
            }
            player.set_volume(15);
            for (order_index, pattern_index) in info.orders[channel].iter().enumerate() {
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
                        if let Some(waveform) = instr.wavetable_synth_data() {
                            let wavedata = self
                                .wavetable(waveform.first_wave as usize)
                                .expect("Invalid wavetable");
                            let index = waveform_indices
                                .get(&wavedata)
                                .expect("Unregistered wavedata");
                            player.set_waveform(*index);
                        }
                        macro_cursor.load(instr, clock.now_tick());
                        for effect in macro_cursor.effects(clock.now_tick()) {
                            effect.apply(&mut player);
                        }
                    }
                    if let Some(note) = row.note {
                        player.start_note(note - 48);
                        empty = false;
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
struct MacroCursor {
    volume: Option<MacroBodyCursor<u8>>,
    arpeggio: Option<MacroBodyCursor<i8>>,
}

impl MacroCursor {
    fn new() -> Self {
        Self {
            volume: None,
            arpeggio: None,
        }
    }

    fn load(&mut self, instr: &FurInstrument, at_tick: u64) {
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

    fn effects(&mut self, until_tick: u64) -> Vec<MacroEffect> {
        let mut effects = BTreeMap::new();
        if let Some(vol) = self.volume.as_mut() {
            for (tick, vol) in vol.values(until_tick) {
                effects.entry(tick).or_insert(MacroEffect::new(tick)).volume = Some(vol);
            }
        }
        if let Some(arp) = self.arpeggio.as_mut() {
            for (tick, arp) in arp.values(until_tick) {
                effects
                    .entry(tick)
                    .or_insert(MacroEffect::new(tick))
                    .arpeggio = Some(arp);
            }
        }
        effects.into_values().collect()
    }
}

#[derive(Debug)]
struct MacroEffect {
    tick: u64,
    volume: Option<u8>,
    arpeggio: Option<i8>,
}
impl MacroEffect {
    fn new(tick: u64) -> Self {
        Self {
            tick,
            volume: None,
            arpeggio: None,
        }
    }
    fn apply(&self, player: &mut ChannelPlayer) {
        if let Some(volume) = self.volume {
            player.set_envelope(volume);
        }
        if let Some(arpeggio) = self.arpeggio {
            player.set_pitch_shift(arpeggio as f64);
        }
    }
}

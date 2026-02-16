mod parser;
mod state;

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
        fur::{
            parser::{FurEffect, FurHeader, FurInfoBlock, FurPatternRow},
            state::FurChannelState,
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
        let patterns = PatternManager::new(info);
        let mut clock = Clock::new(info);
        let mut channels: Vec<FurChannel> = (0..6).map(FurChannel::new).collect();
        let mut orders_seen = HashSet::new();
        let mut order = 0;
        let mut start_row = 0;
        let mut tick = 0;
        'outer: loop {
            if !orders_seen.insert(order) {
                for channel in &mut channels {
                    channel.jump_to(order);
                }
                break;
            }
            for channel in &mut channels {
                channel.start_pattern(order);
            }
            let start = start_row;
            start_row = 0;
            for rows in patterns.iter_all(order).skip(start) {
                let mut next = NextAction::Continue;
                for (channel, row) in channels.iter_mut().zip(rows) {
                    channel.handle_row(row, info);
                    for effect in &row.effects {
                        match effect {
                            FurEffect::JumpToOrder(o) => {
                                next = next.max(NextAction::Jump {
                                    order: *o as usize,
                                    row: 0,
                                });
                            }
                            FurEffect::JumpToNextPattern(r) => {
                                next = next.max(NextAction::Jump {
                                    order: order + 1,
                                    row: *r as u64,
                                });
                            }
                            FurEffect::SetVirtualTempoNumerator(n) => {
                                clock.set_virtual_numerator(tick, *n as u16);
                            }
                            FurEffect::SetVirtualTempoDenominator(d) => {
                                clock.set_virtual_denominator(tick, *d as u16);
                            }
                            FurEffect::StopSong => {
                                next = next.max(NextAction::Stop);
                            }
                            _ => {}
                        }
                    }
                }
                tick += info.speed_1 as u64;
                for channel in &mut channels {
                    channel.advance_time(tick, &clock, info, waveforms)?;
                }
                match next {
                    NextAction::Continue => {}
                    NextAction::Jump { order: o, row } => {
                        order = o;
                        start_row = row as usize;
                        continue 'outer;
                    }
                    NextAction::Stop => {
                        break 'outer;
                    }
                }
            }

            if order == info.orders_length as usize - 1 {
                if self.looping {
                    order = 0
                } else {
                    break;
                }
            } else {
                order += 1;
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
    state: FurChannelState,
    next_tick: u64,
}
impl FurChannel {
    fn new(channel: usize) -> Self {
        let mut player = ChannelPlayer::new(ChannelEffects::default(), false);
        player.set_envelope(15);
        player.set_volume((15, 15));
        player.advance_time(Moment::START);
        Self {
            channel,
            player,
            state: FurChannelState::new(),
            next_tick: 0,
        }
    }
    fn handle_row(&mut self, row: &FurPatternRow, info: &FurInfoBlock) {
        self.state.handle_row(row, info);
    }
    fn advance_time(
        &mut self,
        tick: u64,
        clock: &Clock,
        info: &FurInfoBlock,
        waveforms: &mut WaveformSetData,
    ) -> Result<()> {
        let mut new_tick = self.next_tick;
        for update in self.state.advance(tick - self.next_tick) {
            self.player.advance_time(clock.moment(new_tick));
            new_tick += 1;
            update.apply(&mut self.player, info, waveforms)?;
        }
        self.next_tick = tick;
        Ok(())
    }
    fn start_pattern(&mut self, order: usize) {
        self.player.start_pattern(order as u8);
    }
    fn jump_to(&mut self, order: usize) {
        self.player.go_to_pattern(order as u8);
    }
    fn build(self, name: &str) -> Option<ChannelData> {
        if self.state.is_empty() {
            return None;
        };
        let builder = ChannelBuilder {
            name: format!("{name}_{}", self.channel),
            player: self.player,
        };
        Some(builder.build())
    }
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
enum NextAction {
    Continue,
    Jump { order: usize, row: u64 },
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

struct PatternManager {
    orders: [Vec<u8>; 6],
    patterns: HashMap<(u8, u16), Vec<FurPatternRow>>,
}
impl PatternManager {
    fn new(info: &FurInfoBlock) -> Self {
        let orders = info.orders.clone();
        let mut patterns = HashMap::new();
        for pattern in &info.patterns {
            let mut data = vec![];
            let mut index = 0;
            for row in &pattern.data {
                while index < row.index {
                    data.push(FurPatternRow {
                        index,
                        note: None,
                        instrument: None,
                        volume: None,
                        effects: vec![],
                    });
                    index += 1;
                }
                data.push(row.clone());
                index = row.index + 1;
            }
            while index < info.pattern_length as u64 {
                data.push(FurPatternRow {
                    index,
                    note: None,
                    instrument: None,
                    volume: None,
                    effects: vec![],
                });
                index += 1;
            }
            patterns.insert((pattern.channel, pattern.index), data);
        }
        Self { orders, patterns }
    }

    fn iter(&self, channel: usize, order: usize) -> impl Iterator<Item = &FurPatternRow> {
        let index = self.orders[channel][order];
        let pattern = self.patterns.get(&(channel as u8, index as u16)).unwrap();
        pattern.iter()
    }

    fn iter_all(&self, order: usize) -> impl Iterator<Item = [&FurPatternRow; 6]> {
        Multizip {
            iters: std::array::from_fn(|i| self.iter(i, order)),
        }
    }
}

struct Multizip<I, T, const N: usize>
where
    I: Iterator<Item = T>,
{
    iters: [I; N],
}
impl<I, T, const N: usize> Iterator for Multizip<I, T, N>
where
    I: Iterator<Item = T>,
{
    type Item = [T; N];
    fn next(&mut self) -> Option<Self::Item> {
        let mut result = [const { std::mem::MaybeUninit::<T>::uninit() }; N];
        for (index, iter) in self.iters.iter_mut().enumerate() {
            result[index].write(iter.next()?);
        }
        Some(result.map(|x| unsafe { x.assume_init() }))
    }
}

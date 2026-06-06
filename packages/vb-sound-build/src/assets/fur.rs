mod parser;

use anyhow::{Result, bail};
use binrw::BinRead;
use flate2::bufread::ZlibDecoder;
use std::{
    collections::BTreeMap,
    fs,
    io::{Cursor, Read},
    path::Path,
};

use crate::{
    assets::{
        ChannelData, WaveformSetData,
        fur::parser::{
            FurEffect, FurFeature, FurHeader, FurInfoBlock, FurInstrument, FurInstrumentFile,
            FurMacro, FurMacroBody, FurPatternRow, FurWavetableFile,
        },
        ir::{
            self, Channel, ControlEffect, Effect, Instrument, InstrumentMacro, IrInfo, NoteEvent,
            PanningEffect, Pattern, PatternTick, PitchEffect, VolumeEffect,
        },
    },
    config::ChannelEffects,
};

pub fn decode_waveform(file: &Path) -> Result<[u8; 32]> {
    let bytes = fs::read(file)?;
    let file = FurWavetableFile::read(&mut Cursor::new(bytes))?;
    Ok(file.wavetable.to_waveform())
}

pub fn decode_instrument_file(file: &Path) -> Result<Instrument> {
    let bytes = fs::read(file)?;
    let file = FurInstrumentFile::read(&mut Cursor::new(bytes))?;
    decode_instrument(&file, None)
}

fn decode_instrument(raw: &impl FurInstrument, info: Option<&FurInfoBlock>) -> Result<Instrument> {
    let mut instrument = Instrument::default();
    let mut waveforms = raw.wavetables();
    if let Some(info) = info {
        for (index, wavetable) in info.wavetables.iter().enumerate() {
            waveforms.insert(index, wavetable.to_waveform());
        }
    }
    let load_waveform = |index: usize| match waveforms.get(&index) {
        Some(waveform) => Ok(*waveform),
        None => bail!("invalid waveform {index}"),
    };
    for feature in raw.features() {
        match feature {
            FurFeature::WavetableSynthData(ws) => {
                instrument.waveform = Some(load_waveform(ws.first_wave as usize)?);
            }
            FurFeature::MacroData(md) => {
                for m in md {
                    match m {
                        FurMacro::Volume(mb) => {
                            let m = parse_macro(mb, |v| Ok(*v as f64 / 15.0))?;
                            instrument.volume_macro = Some(m)
                        }
                        FurMacro::Arpeggio(mb) => {
                            let m = parse_macro(mb, |a| Ok(*a))?;
                            instrument.arpeggio_macro = Some(m);
                        }
                        FurMacro::Waveform(mb) => {
                            let m = parse_macro(mb, |i| load_waveform(*i as usize))?;
                            instrument.waveform_macro = Some(m);
                        }
                        FurMacro::Duty(mb) => {
                            let m = parse_macro(mb, |i| Ok(*i))?;
                            instrument.tap_macro = Some(m);
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }
    Ok(instrument)
}

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
        Some(wavetable.to_waveform())
    }

    pub fn decode(self, waveforms: &mut WaveformSetData) -> Result<Vec<ChannelData>> {
        let Self {
            name,
            info,
            looping,
        } = self;
        let mut ir = IrInfo {
            name,
            pattern_length: info.pattern_length as u64 * info.speed_1 as u64,
            ticks_per_second: info.ticks_per_second,
            virtual_tempo_numerator: info.virtual_tempo_numerator,
            virtual_tempo_denominator: info.virtual_tempo_denominator,
            instruments: vec![],
            channels: BTreeMap::new(),
            control: vec![BTreeMap::new(); info.orders[0].len()],
        };
        for raw_instrument in &info.instruments {
            let instrument = decode_instrument(&raw_instrument.value, Some(&info))?;
            ir.instruments.push(instrument);
        }

        let mut patterns: BTreeMap<u8, BTreeMap<usize, Pattern>> = BTreeMap::new();
        for fur_pattern in &info.patterns {
            let mut data = BTreeMap::new();
            for row in &fur_pattern.data {
                let control = parse_row(row, &info, &mut data);
                if !control.is_empty() {
                    for (order, pattern_index) in
                        info.orders[fur_pattern.channel as usize].iter().enumerate()
                    {
                        if *pattern_index != fur_pattern.index as u8 {
                            continue;
                        }
                        let effects = &mut ir.control[order];
                        for (tick, effect) in &control {
                            effects.entry(*tick).or_default().push(effect.clone());
                        }
                    }
                }
            }
            patterns
                .entry(fur_pattern.channel)
                .or_default()
                .insert(fur_pattern.index as usize, Pattern { data });
        }

        for (channel, order) in info.orders.into_iter().enumerate() {
            let channel_patterns = patterns.remove(&(channel as u8)).unwrap();
            ir.channels.insert(
                channel as u8,
                Channel {
                    patterns: channel_patterns,
                    order: order.into_iter().map(|o| o as usize).collect(),
                    effects: ChannelEffects::default(),
                },
            );
        }

        ir::decode(ir, waveforms, looping)
    }
}

fn parse_macro<T1, T2, F>(mb: &FurMacroBody<T1>, map: F) -> Result<InstrumentMacro<T2>>
where
    T1: BinRead,
    for<'a> <T1 as BinRead>::Args<'a>: Default + Clone,
    F: Fn(&T1) -> Result<T2>,
{
    let mut data = vec![];
    for value in &mb.data {
        data.push(map(value)?);
    }
    Ok(InstrumentMacro {
        macro_loop: mb.macro_loop,
        macro_release: mb.macro_release,
        macro_delay: mb.macro_delay,
        macro_speed: mb.macro_speed,
        data,
    })
}

fn parse_row(
    row: &FurPatternRow,
    info: &FurInfoBlock,
    ticks: &mut BTreeMap<u64, PatternTick>,
) -> Vec<(u64, ControlEffect)> {
    assert_eq!(info.linear_pitch, 1);
    let pitch_slide_speed = info.pitch_slide_speed as f64 / 128.0;
    let ticks_per_row = info.speed_1 as u64;

    let first_tick = row.index * ticks_per_row;
    let last_tick = first_tick + ticks_per_row - 1;
    let mut tick = PatternTick {
        note: match row.note {
            Some(182 | 181) => Some(NoteEvent::Release),
            Some(180) => Some(NoteEvent::Stop),
            Some(note) => Some(NoteEvent::Start(note - 48)),
            None => None,
        },
        instrument: row.instrument.map(|i| i as usize),
        volume: row.volume.map(|v| v as f64 / 15.0),
        effects: vec![],
    };
    let parsed = &mut tick.effects;
    let mut control = vec![];
    for effect in row.effects.iter().cloned() {
        match effect {
            FurEffect::Arpeggio(x, y) => parsed.push(Effect::Pitch(PitchEffect::Arpeggio(x, y))),
            FurEffect::PitchSlideUp(speed) => parsed.push(Effect::Pitch(PitchEffect::PitchSlide(
                speed as f64 * pitch_slide_speed,
            ))),
            FurEffect::PitchSlideDown(speed) => parsed.push(Effect::Pitch(
                PitchEffect::PitchSlide(speed as f64 * -pitch_slide_speed),
            )),
            FurEffect::Portamento(speed) => {
                let Some(NoteEvent::Start(note)) = tick.note.take() else {
                    continue;
                };
                parsed.push(Effect::Pitch(PitchEffect::Portamento {
                    note,
                    speed: speed as f64 * pitch_slide_speed,
                }));
            }
            FurEffect::Vibrato(speed, depth) => {
                parsed.push(Effect::Pitch(PitchEffect::Vibrato(speed, depth)))
            }
            FurEffect::SetPanning(l, r) => parsed.push(Effect::Panning(PanningEffect::SetPanning(
                l as f64 / 15.0,
                r as f64 / 15.0,
            ))),
            FurEffect::VolumeSlide(up, down) => parsed.push(Effect::Volume(
                VolumeEffect::VolumeSlide((up as i16 - down as i16) as f64 / 64.0),
            )),
            FurEffect::JumpToOrder(o) => control.push((
                last_tick,
                ControlEffect::Jump {
                    order: o as usize,
                    tick: 0,
                },
            )),
            FurEffect::JumpToNextPattern(t) => control.push((
                last_tick,
                ControlEffect::JumpToNextPattern { tick: t as u64 },
            )),
            FurEffect::SetVolumeLeft(v) => parsed.push(Effect::Panning(
                PanningEffect::SetVolumeLeft(v as f64 / 15.0),
            )),
            FurEffect::SetVolumeRight(v) => parsed.push(Effect::Panning(
                PanningEffect::SetVolumeRight(v as f64 / 15.0),
            )),
            FurEffect::VolumePortamento(speed) => {
                let Some(target) = tick.volume.take() else {
                    continue;
                };
                parsed.push(Effect::Volume(VolumeEffect::VolumePortamento {
                    target,
                    speed: speed as f64 * pitch_slide_speed,
                }));
            }
            FurEffect::ArpeggioSpeed(s) => {
                parsed.push(Effect::Pitch(PitchEffect::ArpeggioSpeed(s)));
            }
            FurEffect::NoteCut(ticks) => parsed.push(Effect::Pitch(PitchEffect::NoteCut(ticks))),
            FurEffect::NoteRelease(ticks) => {
                parsed.push(Effect::Pitch(PitchEffect::NoteRelease(ticks)));
            }
            FurEffect::SetVirtualTempoNumerator(n) => {
                control.push((last_tick, ControlEffect::SetVirtualTempoNumerator(n)));
            }
            FurEffect::SetVirtualTempoDenominator(d) => {
                control.push((last_tick, ControlEffect::SetVirtualTempoDenominator(d)));
            }
            FurEffect::StopSong => control.push((last_tick, ControlEffect::StopSong)),
            FurEffect::Unknown(_, _) => {}
        }
    }
    if tick.note.is_some()
        || tick.instrument.is_some()
        || tick.volume.is_some()
        || !tick.effects.is_empty()
    {
        ticks.insert(first_tick, tick);
    }
    control
}

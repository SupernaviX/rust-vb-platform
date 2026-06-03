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
            FurEffect, FurFeature, FurHeader, FurInfoBlock, FurMacro, FurMacroBody,
            FurWavetableFile,
        },
        ir::{
            self, Channel, ControlEffect, Effect, Instrument, InstrumentMacro, IrInfo, NoteEvent,
            PanningEffect, Pattern, PatternRow, PitchEffect, VolumeEffect,
        },
    },
    config::ChannelEffects,
};

pub fn decode_waveform(file: &Path) -> Result<[u8; 32]> {
    let bytes = fs::read(file)?;
    let file = FurWavetableFile::read(&mut Cursor::new(bytes))?;
    file.wavetable
        .data
        .into_iter()
        .map(|i| i as u8)
        .collect::<Vec<u8>>()
        .try_into()
        .map_err(|v: Vec<u8>| anyhow::anyhow!("invalid wavetable ({} value(s))", v.len()))
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
        assert!(wavetable.data.len() == 32, "Invalid wavetable data");
        let mut result = [0; 32];
        for (index, sample) in wavetable.data.iter().enumerate() {
            result[index] = *sample as u8;
        }
        Some(result)
    }

    pub fn decode(self, waveforms: &mut WaveformSetData) -> Result<Vec<ChannelData>> {
        let Self {
            name,
            info,
            looping,
        } = self;
        let mut ir = IrInfo {
            name,
            pattern_length: info.pattern_length as usize,
            ticks_per_row: info.speed_1,
            ticks_per_second: info.ticks_per_second,
            virtual_tempo_numerator: info.virtual_tempo_numerator,
            virtual_tempo_denominator: info.virtual_tempo_denominator,
            instruments: vec![],
            channels: BTreeMap::new(),
            control: vec![BTreeMap::new(); info.orders[0].len()],
        };
        for raw_instrument in &info.instruments {
            let mut instrument = Instrument {
                waveform: None,
                tap: None,
                volume_macro: None,
                arpeggio_macro: None,
                waveform_macro: None,
                tap_macro: None,
            };
            for feature in &raw_instrument.features {
                match feature {
                    FurFeature::WavetableSynthData(ws) => {
                        instrument.waveform = Some(load_waveform(&info, ws.first_wave as usize)?);
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
                                    let m = parse_macro(mb, |i| load_waveform(&info, *i as usize))?;
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
            ir.instruments.push(instrument);
        }

        let mut patterns: BTreeMap<u8, BTreeMap<usize, Pattern>> = BTreeMap::new();
        for fur_pattern in &info.patterns {
            let mut data = BTreeMap::new();
            for fur_row in &fur_pattern.data {
                let mut note = match fur_row.note {
                    Some(182 | 181) => Some(NoteEvent::Release),
                    Some(180) => Some(NoteEvent::Stop),
                    Some(note) => Some(NoteEvent::Start(note - 48)),
                    None => None,
                };
                let mut volume = fur_row.volume.map(|v| v as f64 / 15.0);
                let (effects, control) =
                    parse_effects(&fur_row.effects, &mut note, &mut volume, &info);
                let row = PatternRow {
                    note,
                    instrument: fur_row.instrument.map(|i| i as usize),
                    volume: fur_row.volume.map(|v| v as f64 / 15.0),
                    effects,
                };
                data.insert(fur_row.index, row);

                if !control.is_empty() {
                    for (order, pattern_index) in
                        info.orders[fur_pattern.channel as usize].iter().enumerate()
                    {
                        if *pattern_index == fur_pattern.index as u8 {
                            ir.control[order]
                                .entry(fur_row.index)
                                .or_default()
                                .extend(control.clone());
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

fn load_waveform(info: &FurInfoBlock, index: usize) -> Result<[u8; 32]> {
    let Some(wavetable) = info.wavetables.get(index) else {
        bail!("Invalid wavetable index {index}");
    };
    if wavetable.data.len() != 32 {
        bail!("Invalid wavetable data");
    }
    let mut result = [0; 32];
    for (index, sample) in wavetable.data.iter().enumerate() {
        result[index] = *sample as u8;
    }
    Ok(result)
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

fn parse_effects(
    effects: &[FurEffect],
    note: &mut Option<NoteEvent>,
    volume: &mut Option<f64>,
    info: &FurInfoBlock,
) -> (Vec<Effect>, Vec<ControlEffect>) {
    assert_eq!(info.linear_pitch, 1);
    let pitch_slide_speed = info.pitch_slide_speed as f64 / 128.0;
    let mut parsed = vec![];
    let mut control = vec![];
    for effect in effects.iter().cloned() {
        match effect {
            FurEffect::Arpeggio(x, y) => parsed.push(Effect::Pitch(PitchEffect::Arpeggio(x, y))),
            FurEffect::PitchSlideUp(speed) => parsed.push(Effect::Pitch(PitchEffect::PitchSlide(
                speed as f64 * pitch_slide_speed,
            ))),
            FurEffect::PitchSlideDown(speed) => parsed.push(Effect::Pitch(
                PitchEffect::PitchSlide(speed as f64 * -pitch_slide_speed),
            )),
            FurEffect::Portamento(speed) => {
                let Some(NoteEvent::Start(note)) = note.take() else {
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
            FurEffect::JumpToOrder(o) => control.push(ControlEffect::Jump {
                order: o as usize,
                row: 0,
            }),
            FurEffect::JumpToNextPattern(r) => {
                control.push(ControlEffect::JumpToNextPattern { row: r as u64 })
            }
            FurEffect::SetVolumeLeft(v) => parsed.push(Effect::Panning(
                PanningEffect::SetVolumeLeft(v as f64 / 15.0),
            )),
            FurEffect::SetVolumeRight(v) => parsed.push(Effect::Panning(
                PanningEffect::SetVolumeRight(v as f64 / 15.0),
            )),
            FurEffect::VolumePortamento(speed) => {
                let Some(target) = volume.take() else {
                    continue;
                };
                parsed.push(Effect::Volume(VolumeEffect::VolumePortamento {
                    target,
                    speed: speed as f64 * pitch_slide_speed,
                }));
            }
            FurEffect::ArpeggioSpeed(s) => {
                parsed.push(Effect::Pitch(PitchEffect::ArpeggioSpeed(s)))
            }
            FurEffect::NoteCut(ticks) => parsed.push(Effect::Pitch(PitchEffect::NoteCut(ticks))),
            FurEffect::NoteRelease(ticks) => {
                parsed.push(Effect::Pitch(PitchEffect::NoteRelease(ticks)))
            }
            FurEffect::SetVirtualTempoNumerator(n) => {
                control.push(ControlEffect::SetVirtualTempoNumerator(n))
            }
            FurEffect::SetVirtualTempoDenominator(d) => {
                control.push(ControlEffect::SetVirtualTempoDenominator(d))
            }
            FurEffect::StopSong => control.push(ControlEffect::StopSong),
            FurEffect::Unknown(_, _) => {}
        }
    }
    (parsed, control)
}

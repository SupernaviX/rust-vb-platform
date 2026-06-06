use std::{collections::BTreeMap, fs, path::Path};

use crate::{
    assets::{
        ChannelData, WaveformSetData,
        ir::{
            self, ControlEffect, Effect, Instrument, IrInfo, NoteEvent, Pattern, PatternTick,
            PitchEffect, VolumeEffect,
        },
    },
    config::ChannelEffects,
};

use anyhow::{Context, Result, anyhow, bail};
use serde::Deserialize;

pub struct BeepBoxDecoder {
    name: String,
    song: BeepBoxJson,
    channels: BTreeMap<u8, Vec<Channel>>,
}
impl BeepBoxDecoder {
    pub fn new(name: &str, file: &Path) -> Result<Self> {
        let bytes = fs::read(file)
            .map_err(|e| anyhow!("could not read beepbox from {}: {}", file.display(), e))?;
        let song = serde_json::from_slice(&bytes).context("could not parse beepbox json")?;
        Ok(Self {
            name: name.to_string(),
            song,
            channels: BTreeMap::new(),
        })
    }

    pub fn pcm_channel(
        &mut self,
        index: u8,
        source: u8,
        waveform: [u8; 32],
        effects: &ChannelEffects,
    ) -> Result<()> {
        let Some(channel) = self.song.channels.get(source as usize) else {
            bail!("Beepbox {} has no channel {source}", self.name);
        };
        let base_volume = channel.instruments.first().map_or(100, |i| i.volume) as f64 / 100.0;
        self.channels.entry(source).or_default().push(Channel {
            index,
            instrument: Instrument {
                waveform: Some(waveform),
                tap: None,
                volume_macro: None,
                arpeggio_macro: None,
                waveform_macro: None,
                tap_macro: None,
            },
            effects: ChannelEffects {
                volume: effects.volume * base_volume,
                ..effects.clone()
            },
        });
        Ok(())
    }

    pub fn noise_channel(
        &mut self,
        index: u8,
        source: u8,
        tap: u8,
        effects: &ChannelEffects,
    ) -> Result<()> {
        let Some(channel) = self.song.channels.get(source as usize) else {
            bail!("Beepbox {} has no channel {source}", self.name);
        };
        let base_volume = channel.instruments.first().map_or(100, |i| i.volume) as f64 / 100.0;
        self.channels.entry(source).or_default().push(Channel {
            index,
            instrument: Instrument {
                waveform: None,
                tap: Some(tap),
                volume_macro: None,
                arpeggio_macro: None,
                waveform_macro: None,
                tap_macro: None,
            },
            effects: ChannelEffects {
                volume: effects.volume * base_volume,
                ..effects.clone()
            },
        });
        Ok(())
    }

    pub fn decode(self, waveforms: &mut WaveformSetData) -> Result<Vec<ChannelData>> {
        let song = self.song;

        let ticks_per_second =
            song.beats_per_minute as f32 * song.ticks_per_beat as f32 * 12.0 / 60.0;
        let mut ir = IrInfo {
            name: self.name,
            pattern_length: song.beats_per_bar as u64 * song.ticks_per_beat as u64 * 12,
            ticks_per_second,
            virtual_tempo_numerator: 1,
            virtual_tempo_denominator: 1,
            instruments: vec![],
            channels: BTreeMap::new(),
            control: vec![const { BTreeMap::new() }; song.intro_bars + song.loop_bars], // TODO
        };
        for (source_index, channels) in self.channels {
            let raw_channel = &song.channels[source_index as usize];
            for channel in channels {
                let instrument = ir.instruments.len();
                ir.instruments.push(channel.instrument);

                let mut patterns = BTreeMap::new();
                patterns.insert(
                    0,
                    Pattern {
                        data: BTreeMap::new(),
                    },
                );
                for (index, raw) in raw_channel.patterns.iter().enumerate() {
                    let pattern = parse_pattern(&song, raw_channel.type_, raw, instrument);
                    patterns.insert(index + 1, pattern);
                }

                ir.channels.insert(
                    channel.index,
                    ir::Channel {
                        patterns,
                        order: raw_channel.sequence.clone(),
                        effects: channel.effects.clone(),
                    },
                );
            }
        }

        if song.loop_bars > 0 {
            let end_index = ir.pattern_length - song.ticks_per_beat as u64;
            let end_effect = ControlEffect::Jump {
                order: song.intro_bars,
                tick: 0,
            };
            if let Some(end) = ir.control.last_mut() {
                end.entry(end_index).or_default().push(end_effect);
            } else {
                ir.control
                    .push_mut(BTreeMap::new())
                    .entry(end_index)
                    .or_default()
                    .push(end_effect);
            }
        }
        ir::decode(ir, waveforms, false)
    }
}

fn parse_pattern(
    song: &BeepBoxJson,
    type_: BeepBoxChannelType,
    raw: &BeepBoxPattern,
    instrument: usize,
) -> Pattern {
    let pattern_length = song.beats_per_bar as u64 * song.ticks_per_beat as u64 * 12;
    let mut pattern = Pattern {
        data: BTreeMap::new(),
    };
    for note in &raw.notes {
        let pitch = match type_ {
            BeepBoxChannelType::Pitch => note.pitches[0] + song.key.to_pitch(),
            BeepBoxChannelType::Drum => note.pitches[0] + song.key.to_pitch() + 69,
        };
        let Some((start, rest)) = note.points.split_first() else {
            continue;
        };
        pattern.data.insert(
            (start.tick * 12.0).round() as u64,
            PatternTick {
                note: Some(NoteEvent::Start(pitch)),
                instrument: Some(instrument),
                volume: Some(start.volume as f64 / 100.0),
                effects: vec![],
            },
        );
        let mut last = start;
        for point in rest {
            let last_entry = pattern
                .data
                .entry((last.tick * 12.0).round() as u64)
                .or_default();
            let time_elapsed = (point.tick - last.tick) * 12.0;
            if point.pitch_bend != last.pitch_bend {
                let note = (pitch as i16 + point.pitch_bend) as u8;
                let pitch_change = (point.pitch_bend - last.pitch_bend).abs() as f64;
                last_entry
                    .effects
                    .push(Effect::Pitch(PitchEffect::Portamento {
                        note,
                        speed: pitch_change / time_elapsed,
                    }));
            }

            if point.volume != last.volume {
                let vol_old = last.volume as f64 / 100.0;
                let vol_new = point.volume as f64 / 100.0;
                let vol_speed = (vol_new - vol_old).abs() / time_elapsed;

                last_entry
                    .effects
                    .push(Effect::Volume(VolumeEffect::VolumePortamento {
                        target: vol_new,
                        speed: vol_speed,
                    }));
            }
            last = point;
        }
        let final_tick = (last.tick * 12.0).round() as u64;
        if final_tick < pattern_length {
            let final_entry = pattern.data.entry(final_tick).or_default();
            final_entry.note = Some(NoteEvent::Stop);
        }
    }
    pattern
}

struct Channel {
    index: u8,
    instrument: Instrument,
    effects: ChannelEffects,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct BeepBoxJson {
    key: BeepBoxKey,
    intro_bars: usize,
    loop_bars: usize,
    beats_per_bar: u16,
    ticks_per_beat: u16,
    beats_per_minute: u16,
    channels: Vec<BeepBoxChannel>,
}

#[derive(Deserialize)]
enum BeepBoxKey {
    C,
    #[serde(rename = "C♯")]
    CSharp,
    D,
    #[serde(rename = "D♯")]
    DSharp,
    E,
    F,
    #[serde(rename = "F♯")]
    FSharp,
    G,
    #[serde(rename = "G♯")]
    GSharp,
    A,
    #[serde(rename = "A♯")]
    ASharp,
    B,
}
impl BeepBoxKey {
    fn to_pitch(&self) -> u8 {
        match self {
            Self::C => 12,
            Self::CSharp => 13,
            Self::D => 14,
            Self::DSharp => 15,
            Self::E => 16,
            Self::F => 17,
            Self::FSharp => 18,
            Self::G => 19,
            Self::GSharp => 20,
            Self::A => 21,
            Self::ASharp => 22,
            Self::B => 23,
        }
    }
}

#[derive(Deserialize)]
struct BeepBoxChannel {
    #[serde(rename = "type")]
    type_: BeepBoxChannelType,
    instruments: Vec<BeepBoxInstrument>,
    patterns: Vec<BeepBoxPattern>,
    sequence: Vec<usize>,
}

#[derive(Deserialize, Clone, Copy)]
#[serde(rename_all = "lowercase")]
enum BeepBoxChannelType {
    Pitch,
    Drum,
}

#[derive(Deserialize)]
struct BeepBoxInstrument {
    volume: u8,
}

#[derive(Deserialize)]
struct BeepBoxPattern {
    notes: Vec<BeepBoxNote>,
}

#[derive(Deserialize)]
struct BeepBoxNote {
    pitches: Vec<u8>,
    points: Vec<BeepBoxNotePoint>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct BeepBoxNotePoint {
    tick: f64,
    pitch_bend: i16,
    volume: u8,
}

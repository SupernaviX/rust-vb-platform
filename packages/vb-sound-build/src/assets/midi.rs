use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    time::Duration,
};

use anyhow::{Result, anyhow};
use midi_reader_writer::midly_0_5::{
    exports::{MetaMessage, MidiMessage, Smf, Timing, TrackEventKind},
    merge_tracks,
};

use crate::assets::{Channel, sound::EventEncoder};

use super::sound::{ChannelPlayer, Moment};

struct ChannelBuilder {
    name: String,
    player: ChannelPlayer,
}
impl ChannelBuilder {
    fn build(self) -> Channel {
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

pub struct MidiDecoder {
    name: String,
    file: PathBuf,
    channels: HashMap<u8, Vec<ChannelBuilder>>,
}
impl MidiDecoder {
    pub fn new(name: &str, file: &Path) -> Self {
        Self {
            name: name.to_string(),
            file: file.to_path_buf(),
            channels: HashMap::new(),
        }
    }

    pub fn pcm_channel(&mut self, name: &str, index: u8, waveform: u8) {
        let mut player = ChannelPlayer::new();
        player.set_instrument(waveform);
        player.set_volume(normalize_volume(127));
        player.set_envelope(normalize_volume(127));
        self.channels
            .entry(index)
            .or_default()
            .push(ChannelBuilder {
                name: format!("{}_{name}", self.name),
                player,
            });
    }

    pub fn noise_channel(&mut self, name: &str, index: u8, tap: u8) {
        let mut player = ChannelPlayer::new();
        player.set_tap(tap);
        player.set_volume(normalize_volume(127));
        player.set_envelope(normalize_volume(127));
        self.channels
            .entry(index)
            .or_default()
            .push(ChannelBuilder {
                name: format!("{}_{name}", self.name),
                player,
            });
    }

    pub fn decode(mut self) -> Result<Vec<Channel>> {
        let bytes = fs::read(&self.file)
            .map_err(|e| anyhow!("could not read midi from {}: {}", self.file.display(), e))?;
        let data = Smf::parse(&bytes)?;
        let mut clock = Clock::new(&data);
        for (ticks, _, event) in merge_tracks(&data.tracks) {
            clock.advance(ticks);
            match event {
                TrackEventKind::Meta(MetaMessage::Tempo(tempo)) => {
                    clock.set_tempo(tempo.as_int());
                }
                TrackEventKind::Meta(MetaMessage::TimeSignature(_, denom, _, _)) => {
                    clock.set_time_signature_denom(denom);
                }
                TrackEventKind::Midi { channel, message } => {
                    let channel = channel.as_int();
                    let Some(channels) = self.channels.get_mut(&channel) else {
                        continue;
                    };
                    for channel in channels.iter_mut() {
                        channel.player.advance_time(clock.now());
                        match &message {
                            MidiMessage::NoteOn { key, vel } => {
                                if vel.as_int() > 0 {
                                    channel.player.start_note(key.as_int());
                                } else {
                                    channel.player.stop_note();
                                }
                            }
                            MidiMessage::NoteOff { .. } => {
                                channel.player.stop_note();
                            }
                            MidiMessage::Controller { controller, value } => {
                                let controller = controller.as_int();
                                let value = value.as_int();
                                match controller {
                                    7 => {
                                        // volume (out of 127)
                                        channel.player.set_volume(normalize_volume(value));
                                    }
                                    11 => {
                                        // "expression" (percentage of volume, out of 127)
                                        channel.player.set_envelope(normalize_volume(value));
                                    }
                                    _ => {}
                                }
                            }
                            _ => {}
                        }
                    }
                }
                _ => {}
            }
        }
        Ok(self
            .channels
            .into_values()
            .flatten()
            .map(|c| c.build())
            .collect())
    }
}

// MIDI range for volume/expression is 0-127,
// and should be squared (as a fraction of 1) to sound right
fn normalize_volume(mid: u8) -> u8 {
    ((mid as f32 / 127.0).powi(2) * 255.0).round() as u8
}

struct Clock {
    timing: Timing,
    tempo: u32,
    time_signature_denom: u32,
    now_ticks: u64,
    elapsed: Duration,
}
impl Clock {
    fn new(data: &Smf) -> Self {
        Self {
            timing: data.header.timing,
            tempo: 500_000,
            time_signature_denom: 4,
            now_ticks: 0,
            elapsed: Duration::ZERO,
        }
    }

    fn advance(&mut self, now_ticks: u64) {
        let ticks = (now_ticks - self.now_ticks) as u32;
        if ticks == 0 {
            return;
        }
        self.now_ticks = now_ticks;
        self.elapsed += self.tick_duration(ticks);
    }

    fn tick_duration(&self, ticks: u32) -> Duration {
        match self.timing {
            Timing::Metrical(ticks_per_beat) => {
                let time_per_beat =
                    Duration::from_micros(self.tempo as u64) * 4 / self.time_signature_denom;
                time_per_beat * ticks / ticks_per_beat.as_int() as u32
            }
            Timing::Timecode(fps, subframes) => {
                let ticks_per_second = fps.as_f32() * subframes as f32;
                Duration::from_secs_f32(ticks as f32 / ticks_per_second)
            }
        }
    }

    fn now(&self) -> Moment {
        Moment::START + self.elapsed
    }

    fn set_tempo(&mut self, tempo: u32) {
        self.tempo = tempo;
    }

    fn set_time_signature_denom(&mut self, denom: u8) {
        self.time_signature_denom = 1u32 << denom;
    }
}

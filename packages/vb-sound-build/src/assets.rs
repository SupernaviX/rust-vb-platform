mod midi;
mod sound;

use std::collections::HashMap;

use anyhow::Result;

use crate::{assets::midi::MidiDecoder, config::RawAssets};

pub fn process(assets: RawAssets) -> Result<Assets> {
    let mut waveforms = vec![];
    let mut waveform_indices = HashMap::new();
    let mut instrument_waveforms = HashMap::new();
    let mut instrument_taps = HashMap::new();
    for (name, instrument) in assets.instruments {
        if let Some(waveform) = instrument.waveform {
            let index = *waveform_indices.entry(waveform).or_insert_with(|| {
                waveforms.push(Waveform { data: waveform });
                waveforms.len() as u8 - 1
            });
            instrument_waveforms.insert(name, index);
        } else if let Some(tap) = instrument.tap {
            instrument_taps.insert(name, tap);
        }
    }
    let mut channels = vec![];
    for (name, midi) in assets.midis {
        let mut decoder = MidiDecoder::new(&name, &midi.file, midi.looping);
        for (name, channel) in midi.channels {
            if let Some(waveform) = instrument_waveforms.get(&channel.instrument) {
                decoder.pcm_channel(&name, channel.channel, *waveform, &channel.effects);
            } else if let Some(tap) = instrument_taps.get(&channel.instrument) {
                decoder.noise_channel(&name, channel.channel, *tap, &channel.effects);
            }
        }
        for channel in decoder.decode()? {
            channels.push(channel);
        }
    }
    channels.sort_by(|c1, c2| c1.name.cmp(&c2.name));
    Ok(Assets {
        waveforms,
        channels,
    })
}

pub struct Assets {
    pub waveforms: Vec<Waveform>,
    pub channels: Vec<Channel>,
}

pub struct Waveform {
    pub data: [u8; 32],
}

pub struct Channel {
    pub name: String,
    pub data: Vec<u8>,
}

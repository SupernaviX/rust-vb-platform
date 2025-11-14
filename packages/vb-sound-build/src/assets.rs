mod fur;
mod midi;
mod sound;

use std::collections::HashMap;

use anyhow::Result;

use crate::{
    assets::{fur::FurDecoder, midi::MidiDecoder},
    config::RawAssets,
};

pub fn process(assets: RawAssets) -> Result<Assets> {
    let mut waveforms = vec![];
    let mut waveform_indices = HashMap::new();
    let mut named_waveforms = HashMap::new();

    let mut furs = HashMap::new();
    for (name, raw) in assets.furs {
        let decoder = FurDecoder::new(&name, &raw.file, raw.looping)?;
        furs.insert(name, decoder);
    }
    for (name, instrument) in assets.waveforms {
        if let Some(fur) = instrument.fur {
            let decoder = furs.get(&fur.name).expect("unrecognized fur");
            let waveform = decoder
                .wavetable(fur.wavetable)
                .expect("unrecognized wavetable");
            let index = *waveform_indices.entry(waveform).or_insert_with(|| {
                waveforms.push(Waveform { data: waveform });
                waveforms.len() as u8 - 1
            });
            named_waveforms.insert(name, index);
        } else if let Some(waveform) = instrument.values {
            let index = *waveform_indices.entry(waveform).or_insert_with(|| {
                waveforms.push(Waveform { data: waveform });
                waveforms.len() as u8 - 1
            });
            named_waveforms.insert(name, index);
        }
    }
    let mut channels = vec![];
    for decoder in furs.into_values() {
        for channel in decoder.decode(&waveform_indices)? {
            channels.push(channel);
        }
    }
    for (name, midi) in assets.midis {
        let mut decoder = MidiDecoder::new(&name, &midi.file, midi.looping);
        for (name, channel) in midi.channels {
            if let Some(waveform_name) = channel.waveform {
                let waveform = named_waveforms
                    .get(&waveform_name)
                    .unwrap_or_else(|| panic!("Unrecognized waveform"));
                decoder.pcm_channel(&name, channel.channel, *waveform, &channel.effects);
            } else if let Some(tap) = channel.tap {
                decoder.noise_channel(&name, channel.channel, tap, &channel.effects);
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

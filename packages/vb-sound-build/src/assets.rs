mod fur;
mod midi;
mod sound;

use std::collections::HashMap;

use anyhow::{Result, bail};

use crate::{
    assets::{fur::FurDecoder, midi::MidiDecoder},
    config::RawAssets,
};

pub fn process(assets: RawAssets) -> Result<Assets> {
    let mut waveform_sets = vec![];
    let mut named_waveforms = HashMap::new();

    let mut furs = HashMap::new();
    for (name, raw) in &assets.furs {
        let decoder = FurDecoder::new(name, &raw.file, raw.looping)?;
        furs.insert(name.clone(), decoder);
    }
    for (name, instrument) in assets.waveforms {
        if let Some(fur) = instrument.fur {
            let decoder = furs.get(&fur.name).expect("unrecognized fur");
            let waveform = decoder
                .wavetable(fur.wavetable)
                .expect("unrecognized wavetable");
            named_waveforms.insert(name, waveform);
        } else if let Some(waveform) = instrument.values {
            named_waveforms.insert(name, waveform);
        }
    }
    let mut channels = vec![];
    for (name, decoder) in furs {
        let raw = assets.furs.get(&name).unwrap();
        let mut waveforms = WaveformSetData::new(name);
        for waveform_name in &raw.fixed_waveforms {
            let waveform = named_waveforms
                .get(waveform_name)
                .copied()
                .unwrap_or_else(|| panic!("Unrecognized waveform \"{waveform_name}\""));
            waveforms.add_waveform(waveform)?;
        }
        for channel in decoder.decode(&mut waveforms)? {
            channels.push(channel);
        }
        waveform_sets.push(waveforms);
    }
    for (name, midi) in assets.midis {
        let mut decoder = MidiDecoder::new(&name, &midi.file, midi.looping);
        let mut waveforms = WaveformSetData::new(name);
        for waveform_name in &midi.fixed_waveforms {
            let waveform = named_waveforms
                .get(waveform_name)
                .copied()
                .unwrap_or_else(|| panic!("Unrecognized waveform \"{waveform_name}\""));
            waveforms.add_waveform(waveform)?;
        }
        for (name, channel) in midi.channels {
            if let Some(waveform_name) = channel.waveform {
                let waveform = named_waveforms
                    .get(&waveform_name)
                    .unwrap_or_else(|| panic!("Unrecognized waveform \"{waveform_name}\""));
                let index = waveforms.add_waveform(*waveform)?;
                decoder.pcm_channel(&name, channel.channel, index, &channel.effects);
            } else if let Some(tap) = channel.tap {
                decoder.noise_channel(&name, channel.channel, tap, &channel.effects);
            }
        }
        for channel in decoder.decode()? {
            channels.push(channel);
        }
        waveform_sets.push(waveforms);
    }
    channels.sort_by(|c1, c2| c1.name.cmp(&c2.name));
    Ok(Assets {
        waveform_sets,
        channels,
    })
}

pub struct Assets {
    pub waveform_sets: Vec<WaveformSetData>,
    pub channels: Vec<ChannelData>,
}

pub struct WaveformSetData {
    pub name: String,
    pub waveforms: Vec<[u8; 32]>,
}
impl WaveformSetData {
    fn new(name: String) -> Self {
        Self {
            name,
            waveforms: vec![],
        }
    }
    fn add_waveform(&mut self, waveform: [u8; 32]) -> Result<u8> {
        match self.waveforms.iter().position(|w| w == &waveform) {
            Some(i) => Ok(i as u8),
            None => {
                let i = self.waveforms.len() as u8;
                if i >= 5 {
                    bail!("too many waveforms");
                }
                self.waveforms.push(waveform);
                Ok(i)
            }
        }
    }

    /// A set of waveforms is serialized as a 4-byte length in bytes,
    /// followed by all da bytes.
    pub fn as_bytes(&self) -> Vec<u8> {
        let mut result = vec![];
        result.extend((self.waveforms.len() as u32 * 32).to_le_bytes());
        for waveform in &self.waveforms {
            result.extend_from_slice(waveform);
        }
        result
    }
}

pub struct ChannelData {
    pub name: String,
    pub data: Vec<u8>,
}

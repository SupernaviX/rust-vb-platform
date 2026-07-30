use std::collections::HashMap;

use crate::{
    assets::{
        fur,
        ir::{Instrument, InstrumentMacro},
    },
    config::RawInstrument,
};

use anyhow::{Result, bail};

pub fn parse_instrument(
    raw: &RawInstrument,
    waveforms: &HashMap<String, [u8; 32]>,
) -> Result<Instrument> {
    let mut instrument = if let Some(file) = &raw.file {
        fur::decode_instrument_file(file)?
    } else {
        Instrument::default()
    };
    if let Some(wav) = &raw.waveform {
        let Some(waveform) = waveforms.get(wav) else {
            bail!("unrecognized waveform {wav}")
        };
        instrument.waveform = Some(*waveform);
    }
    if let Some(vib) = &raw.vibrato {
        instrument.vibrato_macro = Some(compute_vibrato(vib.speed, vib.depth))
    }
    Ok(instrument)
}

fn compute_vibrato(speed: u8, depth: u8) -> InstrumentMacro<f64> {
    let period = 64 / speed as usize;
    let amplitude = depth as f64 / 16.0;
    let data = (0..period)
        .map(|i| {
            let t = i as f64 * std::f64::consts::TAU / period as f64;
            t.sin() * amplitude
        })
        .collect();
    InstrumentMacro {
        macro_loop: 0,
        macro_delay: 0,
        macro_release: -1,
        macro_speed: 1,
        data,
    }
}

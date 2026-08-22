use std::io::Write;

use anyhow::Result;
use serde::Serialize;

use crate::{Options, assets::Assets};

pub fn generate(opts: &mut Options, assets: &Assets) -> Result<Stats> {
    let mut stats = Stats { total_bytes: 0 };

    for waveforms in &assets.waveform_sets {
        stats.total_bytes += waveforms.as_bytes().len();
    }
    for channel in &assets.channels {
        stats.total_bytes += channel.data.len();
    }

    let mut file = opts.output_file("sound_stats.toml")?;
    file.write_all(toml::to_string_pretty(&stats)?.as_bytes())?;
    file.flush()?;

    Ok(stats)
}

#[derive(Serialize)]
pub struct Stats {
    pub total_bytes: usize,
}

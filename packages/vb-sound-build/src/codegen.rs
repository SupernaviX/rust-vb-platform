use std::{io::Write as _, path::MAIN_SEPARATOR};

use crate::{Options, assets::Assets};
use anyhow::Result;

fn include(datatype: &str, filename: &str) -> String {
    format!(
        "vb_sound::include_{}!(\"{}\")",
        datatype,
        filename.escape_default()
    )
}

pub fn generate(opts: &mut Options, assets: Assets) -> Result<()> {
    let mut file = opts.output_file("sound_assets.rs")?;

    for waveforms in &assets.waveform_sets {
        let waveforms_filename = format!("waveforms{MAIN_SEPARATOR}{}.bin", waveforms.name);
        let mut waveforms_file = opts.output_file(&waveforms_filename)?;
        let waveforms_bytes = waveforms.as_bytes();
        waveforms_file.write_all(&waveforms_bytes)?;
        waveforms_file.flush()?;

        writeln!(file, "#[allow(dead_code)]")?;
        writeln!(
            file,
            "pub static {}_WAVEFORMS: vb_sound::WaveformData<{}> = {};",
            rust_identifier(&waveforms.name),
            waveforms_bytes.len(),
            include("waveforms", &waveforms_filename),
        )?;
    }

    for channel in assets.channels {
        let channel_filename = format!("channel{MAIN_SEPARATOR}{}.bin", channel.name);
        let mut channel_file = opts.output_file(&channel_filename)?;
        channel_file.write_all(&channel.data)?;
        channel_file.flush()?;

        writeln!(file, "#[allow(dead_code)]")?;
        writeln!(
            file,
            "pub static {}: [u32; {}] = {};",
            rust_identifier(&channel.name),
            channel.data.len() / 4,
            include("channel", &channel_filename),
        )?;
        writeln!(file)?;
    }

    file.flush()?;
    Ok(())
}

fn rust_identifier(name: &str) -> String {
    name.to_uppercase().replace("-", "_")
}

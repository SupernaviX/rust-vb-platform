use std::io::Write as _;

use crate::{Options, assets::Assets};
use anyhow::Result;

pub fn generate(opts: &Options, assets: Assets) -> Result<()> {
    let mut file = opts.output_file("sound_assets.rs")?;

    for waveforms in &assets.waveform_sets {
        let waveforms_filename = format!("waveforms.{}.bin", waveforms.name);
        let mut waveforms_file = opts.output_file(&waveforms_filename)?;
        let waveforms_bytes = waveforms.as_bytes();
        waveforms_file.write_all(&waveforms_bytes)?;
        waveforms_file.flush()?;

        writeln!(file, "#[allow(dead_code)]")?;
        writeln!(
            file,
            "pub static {}_WAVEFORMS: [u8; {}] = vb_sound::include_waveforms!(\"{}\");",
            rust_identifier(&waveforms.name),
            waveforms_bytes.len(),
            waveforms_filename
        )?;
    }

    for channel in assets.channels {
        let channel_filename = format!("channel.{}.bin", channel.name);
        let mut channel_file = opts.output_file(&channel_filename)?;
        channel_file.write_all(&channel.data)?;
        channel_file.flush()?;

        writeln!(file, "#[allow(dead_code)]")?;
        writeln!(
            file,
            "pub static {}: [u32; {}] = vb_sound::include_channel!(\"{channel_filename}\");",
            rust_identifier(&channel.name),
            channel.data.len() / 4,
        )?;
        writeln!(file)?;
    }

    file.flush()?;
    Ok(())
}

fn rust_identifier(name: &str) -> String {
    name.to_uppercase().replace("-", "_")
}

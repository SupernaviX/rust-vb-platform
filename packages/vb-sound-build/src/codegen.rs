use std::{fmt::Write as _, io::Write as _};

use crate::{Options, assets::Assets};
use anyhow::Result;

pub fn generate(opts: &Options, assets: Assets) -> Result<()> {
    let mut file = opts.output_file("sound_assets.rs")?;

    let mut waveforms = vec![];
    for waveform in &assets.waveforms {
        let mut string = "[".to_string();
        for (i, sample) in waveform.data.iter().enumerate() {
            if i > 0 {
                write!(&mut string, ", {sample}")?;
            } else {
                write!(&mut string, "{sample}")?;
            }
        }
        string.push(']');
        waveforms.push(string);
    }

    writeln!(file, "#[allow(dead_code)]")?;
    if assets.waveforms.is_empty() {
        writeln!(file, "pub static WAVEFORMS: [[u8; 32]; 0] = [];")?;
    } else {
        writeln!(
            file,
            "pub static WAVEFORMS: [[u8; 32]; {}] = [",
            waveforms.len()
        )?;
        for waveform in assets.waveforms {
            write!(file, "    [")?;
            for (i, sample) in waveform.data.iter().enumerate() {
                if i > 0 {
                    write!(file, ", {sample}")?;
                } else {
                    write!(file, "{sample}")?;
                }
            }
            writeln!(file, "],")?;
        }
        writeln!(file, "];")?;
    }
    writeln!(file)?;

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

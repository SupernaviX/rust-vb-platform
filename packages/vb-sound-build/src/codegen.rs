use std::{io::Write, path::MAIN_SEPARATOR};

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

    writeln!(file, "// generated sound data")?;

    generate_module(
        &mut file,
        true,
        "waveform_sets",
        assets.waveform_sets,
        |file, waveforms| {
            let waveforms_filename = format!("waveforms{MAIN_SEPARATOR}{}.bin", waveforms.name);
            let mut waveforms_file = opts.output_file(&waveforms_filename)?;
            let waveforms_bytes = waveforms.as_bytes();
            waveforms_file.write_all(&waveforms_bytes)?;
            waveforms_file.flush()?;

            writeln!(
                file,
                "    pub static {}: vb_sound::WaveformData<{}> = {};",
                rust_identifier(&waveforms.name),
                waveforms_bytes.len(),
                include("waveforms", &waveforms_filename),
            )?;

            Ok(())
        },
    )?;

    generate_module(
        &mut file,
        true,
        "channels",
        assets.channels,
        |file, channel| {
            let channel_filename = format!("channel{MAIN_SEPARATOR}{}.bin", channel.name);
            let mut channel_file = opts.output_file(&channel_filename)?;
            channel_file.write_all(&channel.data)?;
            channel_file.flush()?;

            writeln!(
                file,
                "    pub static {}: [u32; {}] = {};",
                rust_identifier(&channel.name),
                channel.data.len() / 4,
                include("channel", &channel_filename),
            )?;

            Ok(())
        },
    )?;

    file.flush()?;
    Ok(())
}

fn generate_module<T, E, F>(
    file: &mut T,
    allow_dead_code: bool,
    name: &str,
    elements: Vec<E>,
    mut render: F,
) -> Result<()>
where
    T: Write,
    F: FnMut(&mut T, E) -> Result<()>,
{
    if elements.is_empty() {
        return Ok(());
    }
    writeln!(file)?;
    if allow_dead_code {
        writeln!(file, "#[allow(dead_code)]")?;
    }
    writeln!(file, "pub mod {name} {{")?;
    let mut newline = false;
    for element in elements {
        if newline {
            writeln!(file)?;
        }
        newline = true;
        render(file, element)?;
    }
    writeln!(file, "}}")?;
    Ok(())
}

fn rust_identifier(name: &str) -> String {
    name.to_uppercase().replace("-", "_")
}

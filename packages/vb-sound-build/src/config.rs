use std::{
    collections::{BTreeMap, HashSet},
    env,
    fs::File,
    io::BufWriter,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail};
use serde::Deserialize;

pub struct Options {
    config_file: PathBuf,
    input_dir: PathBuf,
    output_dir: PathBuf,
    emit_cargo: bool,
    seen: HashSet<PathBuf>,
}

impl Options {
    pub fn cargo_defaults() -> Result<Self> {
        Ok(Self {
            config_file: PathBuf::from("assets.toml"),
            input_dir: env::current_dir()?,
            output_dir: PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR not defined")),
            emit_cargo: true,
            seen: HashSet::new(),
        })
    }

    pub fn new(input_dir: PathBuf, output_dir: PathBuf) -> Self {
        Self {
            config_file: PathBuf::from("assets.toml"),
            input_dir,
            output_dir,
            emit_cargo: true,
            seen: HashSet::new(),
        }
    }

    pub fn with_input_dir(self, input_dir: PathBuf) -> Self {
        Self { input_dir, ..self }
    }

    fn config_file_path(&mut self) -> PathBuf {
        self.input_path(&self.config_file.clone())
    }

    fn input_path(&mut self, path: &Path) -> PathBuf {
        let result = self.input_dir.join(path);
        if self.emit_cargo && self.seen.insert(result.clone()) {
            println!("cargo:rerun-if-changed={}", result.display());
        }
        result
    }

    pub(crate) fn output_file(&self, path: &str) -> Result<BufWriter<File>> {
        let file = File::create(self.output_dir.join(path))?;
        Ok(BufWriter::new(file))
    }
}

#[derive(Deserialize, Debug)]
struct RawAssetFile {
    #[serde(default)]
    pub imports: Vec<PathBuf>,
    #[serde(rename = "waveform", default)]
    pub waveforms: BTreeMap<String, RawWaveform>,
    #[serde(rename = "midi", default)]
    pub midis: BTreeMap<String, RawMidi>,
    #[serde(rename = "fur", default)]
    pub furs: BTreeMap<String, RawFur>,
}

#[derive(Deserialize, Debug)]
pub struct RawWaveform {
    pub values: Option<[u8; 32]>,
    pub fur: Option<FurWaveform>,
}

#[derive(Deserialize, Debug)]
pub struct FurWaveform {
    pub name: String,
    pub wavetable: usize,
}

const fn default_loop() -> bool {
    true
}

#[derive(Deserialize, Debug)]
pub struct RawMidi {
    pub file: PathBuf,
    #[serde(rename = "loop", default = "default_loop")]
    pub looping: bool,
    #[serde(rename = "channel", default)]
    pub channels: BTreeMap<String, RawChannel>,
    #[serde(default)]
    pub fixed_waveforms: Vec<String>,
}
impl RawMidi {
    fn fix_files(self, opts: &mut Options, dir: &Path) -> Self {
        Self {
            file: opts.input_path(&dir.join(self.file)),
            ..self
        }
    }
}

#[derive(Deserialize, Debug)]
pub struct RawFur {
    pub file: PathBuf,
    #[serde(rename = "loop", default = "default_loop")]
    pub looping: bool,
    #[serde(default)]
    pub fixed_waveforms: Vec<String>,
}
impl RawFur {
    fn fix_files(self, opts: &mut Options, dir: &Path) -> Self {
        Self {
            file: opts.input_path(&dir.join(self.file)),
            ..self
        }
    }
}

#[derive(Deserialize, Debug)]
pub struct RawChannel {
    pub channel: u8,
    pub waveform: Option<String>,
    pub tap: Option<u8>,
    #[serde(flatten, default)]
    pub effects: ChannelEffects,
}

const fn default_shift() -> f64 {
    0.0
}
const fn default_volume() -> f64 {
    1.0
}
#[derive(Deserialize, Debug, Clone)]
pub struct ChannelEffects {
    #[serde(default = "default_shift")]
    pub shift: f64,
    #[serde(default = "default_volume")]
    pub volume: f64,
}
impl Default for ChannelEffects {
    fn default() -> Self {
        Self {
            shift: default_shift(),
            volume: default_volume(),
        }
    }
}

#[derive(Debug)]
pub struct RawAssets {
    pub waveforms: BTreeMap<String, RawWaveform>,
    pub midis: BTreeMap<String, RawMidi>,
    pub furs: BTreeMap<String, RawFur>,
}

pub fn parse(opts: &mut Options) -> Result<RawAssets> {
    let mut assets = RawAssets {
        waveforms: BTreeMap::new(),
        midis: BTreeMap::new(),
        furs: BTreeMap::new(),
    };
    let mut files = vec![opts.config_file_path()];
    while let Some(path) = files.pop() {
        let file = std::fs::read_to_string(&path)
            .with_context(|| format!("could not read config file {}", path.display()))?;
        let file: RawAssetFile = toml::from_str(&file)?;
        let Some(dir) = path.parent() else {
            bail!("invalid config file path {}", path.display());
        };

        for import in file.imports {
            files.push(opts.input_path(&dir.join(import)));
        }

        for (name, instrument) in file.waveforms {
            assets.waveforms.insert(name, instrument);
        }

        for (name, midi) in file.midis {
            assets.midis.insert(name, midi.fix_files(opts, dir));
        }

        for (name, fur) in file.furs {
            assets.furs.insert(name, fur.fix_files(opts, dir));
        }
    }
    Ok(assets)
}

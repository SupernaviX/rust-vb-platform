use std::{
    collections::{BTreeMap, HashSet},
    env,
    fs::File,
    io::BufWriter,
    path::{Path, PathBuf},
};

use anyhow::Result;
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

    pub(crate) fn config_file_path(&mut self) -> PathBuf {
        self.input_path(&self.config_file.clone())
    }

    pub(crate) fn input_path(&mut self, path: &Path) -> PathBuf {
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
pub struct RawAssets {
    #[serde(rename = "image")]
    pub images: BTreeMap<String, RawImage>,
}

#[derive(Deserialize, Debug)]
pub struct RawImage {
    pub chardata: String,
    pub file: PathBuf,
    #[serde(default)]
    pub hflip: bool,
    #[serde(default)]
    pub vflip: bool,
    #[serde(default)]
    pub transpose: bool,
    #[serde(default)]
    pub rotate: usize,
    pub position: Option<(usize, usize)>,
    pub size: Option<(usize, usize)>,
}

pub fn parse(opts: &mut Options) -> Result<RawAssets> {
    let config_path = opts.config_file_path();
    let file = std::fs::read_to_string(&config_path)?;
    let assets: RawAssets = toml::from_str(&file)?;
    Ok(assets)
}

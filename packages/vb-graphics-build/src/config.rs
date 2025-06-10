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
struct RawAssetFile {
    #[serde(default)]
    pub imports: Vec<PathBuf>,
    #[serde(rename = "image", default)]
    pub images: BTreeMap<String, RawImage>,
    #[serde(rename = "mask", default)]
    pub masks: BTreeMap<String, RawMask>,
    #[serde(rename = "font", default)]
    pub fonts: BTreeMap<String, RawFont>,
}

#[derive(Debug)]
pub struct RawAssets {
    pub images: BTreeMap<String, RawImage>,
    pub masks: BTreeMap<String, RawMask>,
    pub fonts: BTreeMap<String, RawFont>,
}

#[derive(Deserialize, Debug)]
pub struct RawImage {
    pub chardata: String,
    #[serde(flatten)]
    pub region: RawImageRegion,
}

#[derive(Deserialize, Debug)]
pub struct RawMask {
    #[serde(flatten)]
    pub region: RawImageRegion,
}

#[derive(Deserialize, Debug)]
pub struct RawImageRegion {
    pub file: PathBuf,
    #[serde(default)]
    pub hflip: bool,
    #[serde(default)]
    pub vflip: bool,
    #[serde(default)]
    pub transpose: bool,
    #[serde(default)]
    pub rotate: usize,
    pub position: Option<(isize, isize)>,
    pub size: Option<(usize, usize)>,
}

#[derive(Deserialize, Debug)]
pub struct RawFont {
    pub file: PathBuf,
    pub size: f32,
}

pub fn parse(opts: &mut Options) -> Result<RawAssets> {
    let mut assets = RawAssets {
        images: BTreeMap::new(),
        masks: BTreeMap::new(),
        fonts: BTreeMap::new(),
    };
    let mut paths = vec![opts.config_file_path()];
    while let Some(path) = paths.pop() {
        let file = std::fs::read_to_string(&path)
            .with_context(|| format!("could not read config file {}", path.display()))?;
        let file: RawAssetFile = toml::from_str(&file)?;
        let Some(dir) = path.parent() else {
            bail!("invalid config file path {}", path.display());
        };

        for import in file.imports {
            paths.push(opts.input_path(&dir.join(import)));
        }

        assets.fonts.extend(file.fonts);
        assets.images.extend(file.images);
        assets.masks.extend(file.masks);
    }
    Ok(assets)
}

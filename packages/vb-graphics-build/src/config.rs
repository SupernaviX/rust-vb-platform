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
    #[serde(default)]
    pub palette: Option<[u8; 3]>,
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
    #[serde(default)]
    pub palette: Option<[u8; 3]>,
    #[serde(flatten)]
    pub region: RawImageRegion,
}
impl RawImage {
    fn fix(self, opts: &mut Options, dir: &Path, palette: Option<[u8; 3]>) -> Self {
        Self {
            palette: self.palette.or(palette),
            region: self.region.fix_files(opts, dir),
            ..self
        }
    }
}

#[derive(Deserialize, Debug)]
pub struct RawMask {
    #[serde(flatten)]
    pub region: RawImageRegion,
}
impl RawMask {
    fn fix_files(self, opts: &mut Options, dir: &Path) -> Self {
        Self {
            region: self.region.fix_files(opts, dir),
        }
    }
}

const fn no_zoom() -> f64 {
    1.0
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
    #[serde(default = "no_zoom")]
    pub scale: f64,
    pub position: Option<(isize, isize)>,
    pub size: Option<(usize, usize)>,
}
impl RawImageRegion {
    fn fix_files(self, opts: &mut Options, dir: &Path) -> Self {
        Self {
            file: opts.input_path(&dir.join(self.file)),
            ..self
        }
    }
}

#[derive(Deserialize, Debug)]
pub struct RawFont {
    pub file: PathBuf,
    pub size: f32,
}
impl RawFont {
    fn fix_files(self, opts: &mut Options, dir: &Path) -> Self {
        Self {
            file: opts.input_path(&dir.join(self.file)),
            ..self
        }
    }
}

pub fn parse(opts: &mut Options) -> Result<RawAssets> {
    let mut assets = RawAssets {
        images: BTreeMap::new(),
        masks: BTreeMap::new(),
        fonts: BTreeMap::new(),
    };
    let mut files = vec![(opts.config_file_path(), None)];
    while let Some((path, parent_palette)) = files.pop() {
        let file = std::fs::read_to_string(&path)
            .with_context(|| format!("could not read config file {}", path.display()))?;
        let file: RawAssetFile = toml::from_str(&file)?;
        let palette = file.palette.or(parent_palette);
        let Some(dir) = path.parent() else {
            bail!("invalid config file path {}", path.display());
        };

        for import in file.imports {
            files.push((opts.input_path(&dir.join(import)), palette));
        }

        for (name, font) in file.fonts {
            assets.fonts.insert(name, font.fix_files(opts, dir));
        }
        for (name, image) in file.images {
            assets.images.insert(name, image.fix(opts, dir, palette));
        }
        for (name, mask) in file.masks {
            assets.masks.insert(name, mask.fix_files(opts, dir));
        }
    }
    Ok(assets)
}

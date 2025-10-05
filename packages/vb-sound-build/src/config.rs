use std::{
    collections::{BTreeMap, HashSet},
    env,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail};
use serde::Deserialize;

pub struct Options {
    config_file: PathBuf,
    input_dir: PathBuf,
    #[expect(unused)]
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
}

#[derive(Deserialize, Debug)]
struct RawAssetFile {
    #[serde(default)]
    pub imports: Vec<PathBuf>,
    #[serde(default)]
    pub sound: BTreeMap<String, RawSound>,
}

#[derive(Deserialize, Debug)]
pub struct RawSound {
    pub file: PathBuf,
}
impl RawSound {
    fn fix_files(self, opts: &mut Options, dir: &Path) -> Self {
        Self {
            file: opts.input_path(&dir.join(self.file)),
        }
    }
}

#[derive(Debug)]
pub struct RawAssets {
    pub sounds: BTreeMap<String, RawSound>,
}

pub fn parse(opts: &mut Options) -> Result<RawAssets> {
    let mut assets = RawAssets {
        sounds: BTreeMap::new(),
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

        for (name, sound) in file.sound {
            assets.sounds.insert(name, sound.fix_files(opts, dir));
        }
    }
    Ok(assets)
}

use std::{
    collections::{BTreeMap, btree_map::Entry},
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Result, anyhow};
use fontdue::{Font, FontSettings};

use crate::assets::Shade;

pub struct TtfAtlas {
    files: BTreeMap<PathBuf, TtfContents>,
}

impl TtfAtlas {
    pub fn new() -> Self {
        Self {
            files: BTreeMap::new(),
        }
    }

    pub fn open(&mut self, full_path: PathBuf) -> Result<&TtfContents> {
        match self.files.entry(full_path) {
            Entry::Occupied(e) => Ok(e.into_mut()),
            Entry::Vacant(e) => {
                let contents = load_ttf_contents(e.key())?;
                Ok(e.insert(contents))
            }
        }
    }
}

pub struct TtfContents {
    font: Font,
}

impl TtfContents {
    pub fn rasterize(&self, character: char, px: f32) -> CharacterData {
        let (metrics, data) = self.font.rasterize(character, px);
        let pixels = data
            .iter()
            .map(|p| match p {
                0..32 => Shade::Transparent,
                32..64 => Shade::Shade1,
                64..128 => Shade::Shade2,
                128.. => Shade::Shade3,
            })
            .collect();
        let mut width = metrics.width;
        if width == 0 {
            width = (px * 0.25) as usize;
        }
        CharacterData {
            width,
            height: metrics.height,
            offset: metrics.ymin,
            pixels,
        }
    }
}

pub struct CharacterData {
    pub width: usize,
    pub height: usize,
    pub offset: i32,
    pub pixels: Vec<Shade>,
}

fn load_ttf_contents(path: &Path) -> Result<TtfContents> {
    let bytes = fs::read(path)?;
    let font = Font::from_bytes(bytes, FontSettings::default()).map_err(|e| anyhow!("{e}"))?;

    Ok(TtfContents { font })
}

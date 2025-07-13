use std::{
    collections::{BTreeMap, btree_map::Entry},
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Result, anyhow};
use fontdue::{Font, FontSettings};

use crate::assets::Shade;

pub struct FontAtlas {
    files: BTreeMap<PathBuf, FontContents>,
}

impl FontAtlas {
    pub fn new() -> Self {
        Self {
            files: BTreeMap::new(),
        }
    }

    pub fn open(&mut self, full_path: PathBuf) -> Result<&FontContents> {
        match self.files.entry(full_path) {
            Entry::Occupied(e) => Ok(e.into_mut()),
            Entry::Vacant(e) => {
                let contents = load_font_contents(e.key())?;
                Ok(e.insert(contents))
            }
        }
    }
}

pub struct FontContents {
    font: Font,
}

impl FontContents {
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

fn load_font_contents(path: &Path) -> Result<FontContents> {
    let bytes = fs::read(path)
        .map_err(|e| anyhow!("could not read font from {}: {}", path.display(), e))?;
    let font = Font::from_bytes(bytes, FontSettings::default()).map_err(|e| anyhow!("{e}"))?;

    Ok(FontContents { font })
}

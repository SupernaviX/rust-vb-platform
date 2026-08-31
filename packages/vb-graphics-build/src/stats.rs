use std::{collections::BTreeMap, io::Write};

use crate::{Options, assets::Assets};
use anyhow::Result;
use serde::Serialize;

pub fn generate(opts: &mut Options, assets: &Assets) -> Result<Stats> {
    let mut stats = Stats {
        total_bytes: 0,
        tiles: BTreeMap::new(),
    };

    for tileset in &assets.tilesets {
        stats.total_bytes += tileset.size_bytes();
        stats
            .tiles
            .insert(tileset.name.clone(), tileset.tiles.len());
    }
    for image in &assets.images {
        stats.total_bytes += image.size_bytes();
    }
    for mask in &assets.masks {
        stats.total_bytes += mask.pixels.len();
    }
    for texture in &assets.textures {
        stats.total_bytes += texture.pixels.len();
    }

    let mut file = opts.output_file("graphics_stats.toml")?;
    file.write_all(toml::to_string_pretty(&stats)?.as_bytes())?;
    file.flush()?;

    Ok(stats)
}

#[derive(Serialize)]
pub struct Stats {
    pub total_bytes: usize,
    pub tiles: BTreeMap<String, usize>,
}

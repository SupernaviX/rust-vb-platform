use anyhow::Result;

use crate::config::RawAssets;

pub fn process(_assets: RawAssets) -> Result<Assets> {
    Ok(Assets {})
}

pub struct Assets {}

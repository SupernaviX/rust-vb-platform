mod codegen;
mod config;
mod image;

use anyhow::Result;
pub use config::Options;

pub fn generate(mut opts: Options) -> Result<()> {
    let raw_assets = config::parse(&mut opts)?;
    let assets = image::process(&mut opts, raw_assets)?;
    codegen::generate(&opts, assets)
}

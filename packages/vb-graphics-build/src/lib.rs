mod assets;
mod codegen;
mod config;

use anyhow::Result;
pub use config::Options;

pub fn generate(mut opts: Options) -> Result<()> {
    let raw_assets = config::parse(&mut opts)?;
    let assets = assets::process(raw_assets)?;
    codegen::generate(&opts, assets)
}

mod assets;
mod codegen;
mod config;
mod stats;

use anyhow::Result;
pub use config::Options;
pub use stats::Stats;

pub fn generate(mut opts: Options) -> Result<Stats> {
    let raw_assets = config::parse(&mut opts)?;
    let assets = assets::process(raw_assets)?;
    let stats = stats::generate(&mut opts, &assets)?;
    codegen::generate(&mut opts, assets)?;
    Ok(stats)
}

mod assets;
mod config;

use anyhow::Result;
pub use config::Options;

pub fn generate(mut opts: Options) -> Result<()> {
    let raw_assets = config::parse(&mut opts)?;
    let _assets = assets::process(raw_assets)?;
    Ok(())
}

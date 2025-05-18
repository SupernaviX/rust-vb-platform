use std::{env, error::Error, fs::File, io::Write as _, path::PathBuf};

pub fn init() -> Result<(), Box<dyn Error>> {
    // build directory for this crate
    let out_dir = PathBuf::from(env::var_os("OUT_DIR").unwrap());

    // put `link.x` in the build directory
    let link_file = out_dir.join("link.x");
    File::create(&link_file)?.write_all(include_bytes!("../link.x"))?;

    // Use `link.x`` as a linker script
    println!("cargo:rustc-link-arg=-T{}", link_file.display());

    Ok(())
}

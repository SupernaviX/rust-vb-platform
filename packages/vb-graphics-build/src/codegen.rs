use std::io::Write;

use crate::{Options, image::Assets};
use anyhow::Result;

pub fn generate(opts: &Options, assets: Assets) -> Result<()> {
    let mut file = opts.output_file("assets.rs")?;

    for chardata in assets.chardata {
        let char_count = chardata.chars.len();
        let chardata_filename = format!("chardata.{}.bin", chardata.name);
        let mut chardata_file = opts.output_file(&chardata_filename)?;
        for char in chardata.chars.into_flattened() {
            chardata_file.write_all(&char.to_le_bytes())?;
        }
        chardata_file.flush()?;

        writeln!(file, "#[allow(dead_code)]")?;
        writeln!(
            file,
            "pub const {}: [vb_rt::sys::vip::Character; {}] = vb_graphics::include_chardata!(\"{}\");",
            rust_identifier(&chardata.name),
            char_count,
            chardata_filename
        )?;
        writeln!(file)?;
    }

    for image in assets.images {
        let cell_count = image.cells.len();
        let celldata_filename = format!("cells.{}.bin", image.name);
        let mut celldata_file = opts.output_file(&celldata_filename)?;
        for cell in image.cells {
            celldata_file.write_all(&cell.to_le_bytes())?;
        }
        celldata_file.flush()?;

        writeln!(
            file,
            "const {}_CELLS: [vb_rt::sys::vip::BGCell; {}] = vb_graphics::include_celldata!(\"{}\");",
            rust_identifier(&image.name),
            cell_count,
            celldata_filename
        )?;
        writeln!(file, "#[allow(dead_code)]")?;
        writeln!(
            file,
            "pub const {}: vb_graphics::Image = vb_graphics::Image {{",
            rust_identifier(&image.name)
        )?;
        writeln!(file, "    width_cells: {},", image.width.div_ceil(8))?;
        writeln!(file, "    height_cells: {},", image.height.div_ceil(8))?;
        writeln!(file, "    data: &{}_CELLS,", rust_identifier(&image.name))?;
        writeln!(file, "}};")?;
        writeln!(file)?;
    }

    file.flush()?;
    Ok(())
}

fn rust_identifier(name: &str) -> String {
    name.to_uppercase().replace("-", "_")
}

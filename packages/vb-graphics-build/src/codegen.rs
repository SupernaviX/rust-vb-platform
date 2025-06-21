use std::io::Write;

use crate::{Options, assets::Assets};
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
            "pub static {}: [vb_rt::sys::vip::Character; {}] = vb_graphics::include_chardata!(\"{}\");",
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
            "static {}_CELLS: [vb_rt::sys::vip::Cell; {}] = vb_graphics::include_celldata!(\"{}\");",
            rust_identifier(&image.name),
            cell_count,
            celldata_filename,
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

    for mask in assets.masks {
        let maskdata_filename = format!("mask.{}.bin", mask.name);
        let mut maskdata_file = opts.output_file(&maskdata_filename)?;
        maskdata_file.write_all(&mask.pixels)?;
        maskdata_file.flush()?;

        writeln!(file, "#[allow(dead_code)]")?;
        writeln!(
            file,
            "pub const {}: vb_graphics::Mask = vb_graphics::Mask {{",
            rust_identifier(&mask.name)
        )?;
        writeln!(file, "    width: {},", mask.width)?;
        writeln!(file, "    height: {},", mask.height)?;
        writeln!(
            file,
            "    data: vb_graphics::include_maskdata!(\"{maskdata_filename}\"),"
        )?;
        writeln!(file, "}};")?;
        writeln!(file)?;
    }

    for texture in assets.textures {
        let texturedata_filename = format!("texture.{}.bin", texture.name);
        let mut texturedata_file = opts.output_file(&texturedata_filename)?;
        texturedata_file.write_all(&texture.pixels)?;
        texturedata_file.flush()?;

        writeln!(file, "#[allow(dead_code)]")?;
        writeln!(
            file,
            "pub const {}: vb_graphics::Texture = vb_graphics::Texture {{",
            rust_identifier(&texture.name),
        )?;
        writeln!(file, "    width: {},", texture.width)?;
        writeln!(file, "    height: {},", texture.height)?;
        writeln!(
            file,
            "    data: vb_graphics::include_texturedata!(\"{texturedata_filename}\"),",
        )?;
        writeln!(file, "}};")?;
        writeln!(file)?;
    }

    for font in assets.fonts {
        let fontdata_filename = format!("font.{}.bin", font.name);
        let mut fontdata_file = opts.output_file(&fontdata_filename)?;
        for char in &font.chars {
            fontdata_file.write_all(&char.as_bytes())?;
        }
        fontdata_file.flush()?;

        writeln!(
            file,
            "static {}_CHARDATA: [vb_graphics::FontCharacter; {}] = vb_graphics::include_fontdata!(\"{}\");",
            rust_identifier(&font.name),
            font.chars.len(),
            fontdata_filename,
        )?;
        writeln!(file, "#[allow(dead_code)]")?;
        writeln!(
            file,
            "pub const {}: vb_graphics::Font = vb_graphics::Font {{",
            rust_identifier(&font.name),
        )?;
        writeln!(
            file,
            "    texture: &{},",
            rust_identifier(&font.texture_name)
        )?;
        writeln!(
            file,
            "    chars: &{}_CHARDATA,",
            rust_identifier(&font.name)
        )?;
        writeln!(file, "    line_height: {},", font.line_height)?;
        writeln!(file, "}};")?;
        writeln!(file)?;
    }

    file.flush()?;
    Ok(())
}

fn rust_identifier(name: &str) -> String {
    name.to_uppercase().replace("-", "_")
}

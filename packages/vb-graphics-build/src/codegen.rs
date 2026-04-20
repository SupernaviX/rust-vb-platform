use std::io::Write;

use crate::{
    Options,
    assets::{Assets, BgSpriteKind, FrameData},
};
use anyhow::Result;

pub fn generate(opts: &Options, assets: Assets) -> Result<()> {
    let mut file = opts.output_file("graphics_assets.rs")?;

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
        generate_frame_cells(&mut file, opts, &image.name, &image.frame)?;
        writeln!(file, "#[allow(dead_code)]")?;
        let (struct_name, stereo) = match &image.frame {
            FrameData::Mono(_) => ("vb_graphics::Image", false),
            FrameData::Stereo { .. } => ("vb_graphics::StereoImage", true),
        };
        writeln!(
            file,
            "pub const {}: {struct_name} = {struct_name} {{",
            rust_identifier(&image.name)
        )?;
        writeln!(file, "    width_cells: {},", image.width.div_ceil(8))?;
        writeln!(file, "    height_cells: {},", image.height.div_ceil(8))?;
        if stereo {
            writeln!(file, "    left: &{}_L_CELLS,", rust_identifier(&image.name))?;
            writeln!(
                file,
                "    right: &{}_R_CELLS,",
                rust_identifier(&image.name)
            )?;
        } else {
            writeln!(file, "    data: &{}_CELLS,", rust_identifier(&image.name))?;
        }
        writeln!(file, "}};")?;
        writeln!(file)?;
    }

    for animation in assets.animations {
        for (index, frame) in animation.frames.iter().enumerate() {
            generate_frame_cells(
                &mut file,
                opts,
                &format!("{}_{}", animation.name, index),
                frame,
            )?;
        }
        let (struct_name, stereo) = match &animation.frames[0] {
            FrameData::Mono(_) => ("vb_graphics::Image", false),
            FrameData::Stereo { .. } => ("vb_graphics::StereoImage", true),
        };
        writeln!(file, "#[allow(dead_code)]")?;
        writeln!(
            file,
            "pub const {}: [{struct_name}; {}] = [",
            rust_identifier(&animation.name),
            animation.frames.len()
        )?;
        for index in 0..animation.frames.len() {
            writeln!(file, "    {struct_name} {{")?;
            writeln!(
                file,
                "        width_cells: {},",
                animation.width.div_ceil(8)
            )?;
            writeln!(
                file,
                "        height_cells: {},",
                animation.height.div_ceil(8)
            )?;
            if stereo {
                writeln!(
                    file,
                    "        left: &{}_{}_L_CELLS,",
                    rust_identifier(&animation.name),
                    index
                )?;
                writeln!(
                    file,
                    "        right: &{}_{}_R_CELLS,",
                    rust_identifier(&animation.name),
                    index
                )?;
            } else {
                writeln!(
                    file,
                    "        data: &{}_{}_CELLS,",
                    rust_identifier(&animation.name),
                    index
                )?;
            }
            writeln!(file, "    }},")?;
        }
        writeln!(file, "];")?;
        writeln!(file)?;
    }

    for bg_sprite_map in assets.bg_sprite_maps {
        writeln!(file, "pub mod {} {{", bg_sprite_map.name.replace("-", "_"))?;
        for sprite in &bg_sprite_map.sprites {
            let name = rust_identifier(&sprite.name);
            match &sprite.kind {
                BgSpriteKind::Image(data) => {
                    writeln!(
                        file,
                        "    pub const {name}: vb_graphics::BgSprite = vb_graphics::BgSprite {{"
                    )?;
                    writeln!(file, "        bgmap: {},", sprite.bgmap)?;
                    writeln!(file, "        x: {},", sprite.x)?;
                    writeln!(file, "        y: {},", sprite.y)?;
                    writeln!(file, "        stereo: {},", sprite.stereo)?;
                    writeln!(file, "        width: {},", data.width)?;
                    writeln!(file, "        height: {},", data.height)?;
                    writeln!(file, "    }};")?;
                }
                BgSpriteKind::Region(data) => {
                    writeln!(
                        file,
                        "    pub const {name}: vb_graphics::BgSprite = {}.region(({}, {}), ({}, {}));",
                        rust_identifier(&data.parent),
                        data.x,
                        data.y,
                        data.width,
                        data.height
                    )?;
                }
                BgSpriteKind::Animation(data) => {
                    writeln!(
                        file,
                        "    pub const {name}: vb_graphics::BgAnimation = vb_graphics::BgAnimation {{"
                    )?;
                    writeln!(file, "        bgmap: {},", sprite.bgmap)?;
                    writeln!(file, "        x: {},", sprite.x)?;
                    writeln!(file, "        y: {},", sprite.y)?;
                    writeln!(file, "        stereo: {},", sprite.stereo)?;
                    writeln!(file, "        frame_width: {},", data.frame_width)?;
                    writeln!(file, "        frame_height: {},", data.frame_height)?;
                    writeln!(file, "        columns: {},", data.columns)?;
                    writeln!(file, "        rows: {},", data.rows)?;
                    writeln!(file, "    }};")?;
                }
            }
        }
        if !bg_sprite_map.chardatas.is_empty() {
            writeln!(file)?;
        }
        for chardata in bg_sprite_map.chardatas {
            writeln!(
                file,
                "    pub fn load_{}(char_offset: u16) {{",
                chardata.replace("-", "_")
            )?;
            for sprite in &bg_sprite_map.sprites {
                let Some(image) = &sprite.image else {
                    continue;
                };
                if image.chardata == chardata {
                    writeln!(
                        file,
                        "        {}.load(super::{}, char_offset);",
                        rust_identifier(&sprite.name),
                        rust_identifier(&image.name)
                    )?;
                }
            }
            writeln!(file, "    }}")?;
        }
        writeln!(file, "}}")?;
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

fn generate_frame_cells<T>(
    file: &mut T,
    opts: &Options,
    name: &str,
    frame: &FrameData,
) -> Result<()>
where
    T: Write,
{
    match frame {
        FrameData::Mono(cells) => generate_cells(file, opts, name, cells),
        FrameData::Stereo { left, right } => {
            generate_cells(file, opts, &format!("{name}_l"), left)?;
            generate_cells(file, opts, &format!("{name}_r"), right)?;
            Ok(())
        }
    }
}

fn generate_cells<T>(file: &mut T, opts: &Options, name: &str, cells: &[u16]) -> Result<()>
where
    T: Write,
{
    let cell_count = cells.len();
    let celldata_filename = format!("cells.{}.bin", name);
    let mut celldata_file = opts.output_file(&celldata_filename)?;
    for cell in cells {
        celldata_file.write_all(&cell.to_le_bytes())?;
    }
    celldata_file.flush()?;

    writeln!(
        file,
        "static {}_CELLS: [vb_rt::sys::vip::Cell; {}] = vb_graphics::include_celldata!(\"{}\");",
        rust_identifier(name),
        cell_count,
        celldata_filename,
    )?;
    Ok(())
}

fn rust_identifier(name: &str) -> String {
    name.to_uppercase().replace("-", "_")
}

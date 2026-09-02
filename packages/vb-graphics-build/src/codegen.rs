use std::{io::Write, path::MAIN_SEPARATOR};

use crate::{
    Options,
    assets::{Assets, BgSpriteKind, FrameData, Frames},
};
use anyhow::Result;

fn include(datatype: &str, filename: &str) -> String {
    format!(
        "vb_graphics::include_{}!(\"{}\")",
        datatype,
        filename.escape_default()
    )
}

pub fn generate(opts: &mut Options, assets: Assets) -> Result<()> {
    let mut file = opts.output_file("graphics_assets.rs")?;

    writeln!(file, "// generated graphical data")?;

    generate_module(
        &mut file,
        true,
        "tilesets",
        assets.tilesets,
        |file, tileset| {
            generate_tileset(file, opts, true, &tileset.name, &tileset.tiles)?;
            Ok(())
        },
    )?;

    generate_module(&mut file, true, "images", assets.images, |file, image| {
        let (frames, animation) = match &image.frames {
            Frames::Static(frame) => {
                generate_frame_cells(file, opts, &image.name, frame)?;
                (std::slice::from_ref(frame), false)
            }
            Frames::Animation(frames) => {
                for (index, frame) in frames.iter().enumerate() {
                    generate_frame_cells(file, opts, &format!("{}_{}", image.name, index), frame)?;
                }
                (frames.as_slice(), true)
            }
        };
        let (struct_name, write_cells, stereo) = match frames[0] {
            FrameData::Mono { tiles: None, .. } => ("vb_graphics::Image", false, false),
            FrameData::Mono { .. } => ("vb_graphics::StandaloneImage", true, false),
            FrameData::Stereo { tiles: None, .. } => ("vb_graphics::StereoImage", false, true),
            FrameData::Stereo { .. } => ("vb_graphics::StandaloneStereoImage", true, true),
        };
        let (start_indent, end_indent) = if animation {
            writeln!(
                file,
                "    pub const {}: [{struct_name}; {}] = [",
                rust_identifier(&image.name),
                frames.len()
            )?;
            ("        ", "        ")
        } else {
            write!(
                file,
                "    pub const {}: {struct_name} = ",
                rust_identifier(&image.name)
            )?;
            ("", "    ")
        };
        for index in 0..frames.len() {
            let line_indent = format!("{end_indent}    ");
            let cell_prefix = if animation {
                format!("{}_{index}", rust_identifier(&image.name))
            } else {
                rust_identifier(&image.name)
            };
            writeln!(file, "{start_indent}{struct_name} {{")?;
            if write_cells {
                writeln!(file, "{line_indent}tiles: &{cell_prefix}_TILES,")?;
            }
            writeln!(
                file,
                "{line_indent}width_cells: {},",
                image.width.div_ceil(8)
            )?;
            writeln!(
                file,
                "{line_indent}height_cells: {},",
                image.height.div_ceil(8)
            )?;
            if stereo {
                writeln!(file, "{line_indent}left: &{cell_prefix}_L_CELLS,")?;
                writeln!(file, "{line_indent}right: &{cell_prefix}_R_CELLS,")?;
            } else {
                writeln!(file, "{line_indent}cells: &{cell_prefix}_CELLS,")?;
            }
            if animation {
                writeln!(file, "{end_indent}}},")?;
            } else {
                writeln!(file, "{end_indent}}};")?;
            }
        }
        if animation {
            writeln!(file, "    ];")?;
        }
        Ok(())
    })?;

    generate_module(
        &mut file,
        false,
        "atlases",
        assets.bg_atlases,
        |file, bg_atlas| {
            writeln!(file, "    pub mod {} {{", bg_atlas.name.replace("-", "_"))?;
            for sprite in &bg_atlas.sprites {
                let name = rust_identifier(&sprite.name);
                match &sprite.kind {
                    BgSpriteKind::Image(data) => {
                        writeln!(
                            file,
                            "        pub const {name}: vb_graphics::BgSprite = vb_graphics::BgSprite {{"
                        )?;
                        writeln!(file, "            bgmap: {},", sprite.bgmap)?;
                        writeln!(file, "            x: {},", sprite.x)?;
                        writeln!(file, "            y: {},", sprite.y)?;
                        writeln!(file, "            stereo: {},", sprite.stereo)?;
                        writeln!(file, "            width: {},", data.width)?;
                        writeln!(file, "            height: {},", data.height)?;
                        writeln!(file, "        }};")?;
                    }
                    BgSpriteKind::Region(data) => {
                        writeln!(
                            file,
                            "        pub const {name}: vb_graphics::BgSprite = {}.region(({}, {}), ({}, {}));",
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
                            "        pub const {name}: vb_graphics::BgAnimation = vb_graphics::BgAnimation {{"
                        )?;
                        writeln!(file, "            bgmap: {},", sprite.bgmap)?;
                        writeln!(file, "            x: {},", sprite.x)?;
                        writeln!(file, "            y: {},", sprite.y)?;
                        writeln!(file, "            stereo: {},", sprite.stereo)?;
                        writeln!(file, "            frame_width: {},", data.frame_width)?;
                        writeln!(file, "            frame_height: {},", data.frame_height)?;
                        writeln!(file, "            columns: {},", data.columns)?;
                        writeln!(file, "            rows: {},", data.rows)?;
                        writeln!(file, "        }};")?;
                    }
                }
            }
            if !bg_atlas.tilesets.is_empty() {
                writeln!(file)?;
            }
            for tileset in bg_atlas.tilesets {
                writeln!(
                    file,
                    "        pub fn load_{}(char_offset: u16) {{",
                    tileset.replace("-", "_")
                )?;
                for sprite in &bg_atlas.sprites {
                    let Some(image) = &sprite.image else {
                        continue;
                    };
                    let load_method = if image.stereo { "load_stereo" } else { "load" };
                    if image.tileset.as_ref().is_some_and(|c| c == &tileset) {
                        writeln!(
                            file,
                            "            {}.{load_method}(super::super::images::{}, char_offset);",
                            rust_identifier(&sprite.name),
                            rust_identifier(&image.name)
                        )?;
                    }
                }
                writeln!(file, "        }}")?;
            }
            writeln!(file, "    }}")?;
            Ok(())
        },
    )?;

    generate_module(&mut file, true, "masks", assets.masks, |file, mask| {
        let maskdata_filename = format!("mask{MAIN_SEPARATOR}{}.bin", mask.name);
        let mut maskdata_file = opts.output_file(&maskdata_filename)?;
        maskdata_file.write_all(&mask.pixels)?;
        maskdata_file.flush()?;

        writeln!(
            file,
            "    pub const {}: vb_graphics::Mask = vb_graphics::Mask {{",
            rust_identifier(&mask.name)
        )?;
        writeln!(file, "        width: {},", mask.width)?;
        writeln!(file, "        height: {},", mask.height)?;
        writeln!(
            file,
            "        data: {},",
            include("maskdata", &maskdata_filename)
        )?;
        writeln!(file, "    }};")?;
        Ok(())
    })?;

    generate_module(
        &mut file,
        true,
        "textures",
        assets.textures,
        |file, texture| {
            let texturedata_filename = format!("texture{MAIN_SEPARATOR}{}.bin", texture.name);
            let mut texturedata_file = opts.output_file(&texturedata_filename)?;
            texturedata_file.write_all(&texture.pixels)?;
            texturedata_file.flush()?;

            writeln!(
                file,
                "    pub const {}: vb_graphics::Texture = vb_graphics::Texture {{",
                rust_identifier(&texture.name),
            )?;
            writeln!(file, "        width: {},", texture.width)?;
            writeln!(file, "        height: {},", texture.height)?;
            writeln!(
                file,
                "        data: {},",
                include("texturedata", &texturedata_filename)
            )?;
            writeln!(file, "    }};")?;

            Ok(())
        },
    )?;

    generate_module(&mut file, true, "fonts", assets.fonts, |file, font| {
        let fontdata_filename = format!("font{MAIN_SEPARATOR}{}.bin", font.name);
        let mut fontdata_file = opts.output_file(&fontdata_filename)?;
        for char in &font.chars {
            fontdata_file.write_all(&char.as_bytes())?;
        }
        fontdata_file.flush()?;

        writeln!(
            file,
            "    static {}_CHARDATA: [vb_graphics::FontCharacter; {}] = {};",
            rust_identifier(&font.name),
            font.chars.len(),
            include("fontdata", &fontdata_filename),
        )?;

        writeln!(
            file,
            "    pub const {}: vb_graphics::Font = vb_graphics::Font {{",
            rust_identifier(&font.name),
        )?;
        writeln!(
            file,
            "        texture: &super::textures::{},",
            rust_identifier(&font.texture_name)
        )?;
        writeln!(
            file,
            "        chars: &{}_CHARDATA,",
            rust_identifier(&font.name)
        )?;
        writeln!(file, "        line_height: {},", font.line_height)?;
        writeln!(file, "    }};")?;

        Ok(())
    })?;

    file.flush()?;
    Ok(())
}

fn generate_module<T, E, F>(
    file: &mut T,
    allow_dead_code: bool,
    name: &str,
    elements: Vec<E>,
    mut render: F,
) -> Result<()>
where
    T: Write,
    F: FnMut(&mut T, E) -> Result<()>,
{
    if elements.is_empty() {
        return Ok(());
    }
    writeln!(file)?;
    if allow_dead_code {
        writeln!(file, "#[allow(dead_code)]")?;
    }
    writeln!(file, "pub mod {name} {{")?;
    let mut newline = false;
    for element in elements {
        if newline {
            writeln!(file)?;
        }
        newline = true;
        render(file, element)?;
    }
    writeln!(file, "}}")?;
    Ok(())
}

fn generate_frame_cells<T>(
    file: &mut T,
    opts: &mut Options,
    name: &str,
    frame: &FrameData,
) -> Result<()>
where
    T: Write,
{
    match frame {
        FrameData::Mono { cells, tiles } => {
            if let Some(tiles) = tiles {
                generate_tileset(file, opts, false, &format!("{name}_tiles"), tiles)?;
            }
            generate_cells(file, opts, name, cells)?;
            Ok(())
        }
        FrameData::Stereo { left, right, tiles } => {
            if let Some(tiles) = tiles {
                generate_tileset(file, opts, false, &format!("{name}_tiles"), tiles)?;
            }
            generate_cells(file, opts, &format!("{name}_l"), left)?;
            generate_cells(file, opts, &format!("{name}_r"), right)?;
            Ok(())
        }
    }
}

fn generate_cells<T>(file: &mut T, opts: &mut Options, name: &str, cells: &[u16]) -> Result<()>
where
    T: Write,
{
    let cell_count = cells.len();
    let celldata_filename = format!("cells{MAIN_SEPARATOR}{}.bin", name);
    let mut celldata_file = opts.output_file(&celldata_filename)?;
    for cell in cells {
        celldata_file.write_all(&cell.to_le_bytes())?;
    }
    celldata_file.flush()?;

    writeln!(
        file,
        "    static {}_CELLS: [vb_rt::sys::vip::Cell; {}] = {};",
        rust_identifier(name),
        cell_count,
        include("celldata", &celldata_filename),
    )?;
    Ok(())
}

fn generate_tileset<T>(
    file: &mut T,
    opts: &mut Options,
    public: bool,
    name: &str,
    tiles: &[[u16; 8]],
) -> Result<()>
where
    T: Write,
{
    let tile_count = tiles.len();
    let tileset_filename = format!("tilesets{MAIN_SEPARATOR}{}.bin", name);
    let mut tileset_file = opts.output_file(&tileset_filename)?;
    for word in tiles.as_flattened() {
        tileset_file.write_all(&word.to_le_bytes())?;
    }
    tileset_file.flush()?;

    writeln!(
        file,
        "    {}static {}: [vb_rt::sys::vip::Character; {}] = {};",
        if public { "pub " } else { "" },
        rust_identifier(name),
        tile_count,
        include("tilesetdata", &tileset_filename),
    )?;
    Ok(())
}

fn rust_identifier(name: &str) -> String {
    name.to_uppercase().replace("-", "_")
}

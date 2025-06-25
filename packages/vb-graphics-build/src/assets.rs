mod png;
mod ttf;

use std::collections::BTreeMap;

use crate::{
    assets::{png::PngContents, ttf::TtfAtlas},
    config::{RawAssets, RawFont, RawImage, RawImageRegion, RawMask},
};
use anyhow::{Result, bail};
use bitfield_struct::bitfield;
use png::PngAtlas;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shade {
    Transparent,
    Black,
    Shade1,
    Shade2,
    Shade3,
}

pub fn process(assets: RawAssets) -> Result<Assets> {
    AssetProcessor::new().process(assets)
}

struct AssetProcessor {
    pngs: PngAtlas,
    ttfs: TtfAtlas,
    chardata: BTreeMap<String, CharData>,
    imagedata: BTreeMap<String, ImageData>,
    maskdata: BTreeMap<String, MaskData>,
    texturedata: BTreeMap<String, TextureData>,
    fontdata: BTreeMap<String, FontData>,
}

impl AssetProcessor {
    pub fn new() -> Self {
        Self {
            pngs: PngAtlas::new(),
            ttfs: TtfAtlas::new(),
            chardata: BTreeMap::new(),
            imagedata: BTreeMap::new(),
            maskdata: BTreeMap::new(),
            texturedata: BTreeMap::new(),
            fontdata: BTreeMap::new(),
        }
    }

    pub fn process(mut self, assets: RawAssets) -> Result<Assets> {
        for (name, image) in assets.images {
            self.process_image(name, image)?;
        }
        for (name, mask) in assets.masks {
            self.process_mask(name, mask)?;
        }
        for (name, font) in assets.fonts {
            self.process_font(name, font)?;
        }
        Ok(Assets {
            chardata: self.chardata.into_values().collect(),
            images: self.imagedata.into_values().collect(),
            masks: self.maskdata.into_values().collect(),
            textures: self.texturedata.into_values().collect(),
            fonts: self.fontdata.into_values().collect(),
        })
    }

    fn process_image(&mut self, name: String, image: RawImage) -> Result<()> {
        let png = self.pngs.open(image.region.file.to_path_buf())?;
        let ImageRegion {
            position,
            size,
            transform,
        } = parse_region(png, &image.region)?;
        let view = png.view(position, size, transform);

        let chardata = self
            .chardata
            .entry(image.chardata)
            .or_insert_with_key(|name| CharData {
                name: name.clone(),
                chars: vec![[0; 8]],
            });

        let mut cells = vec![];
        let (width, height) = view.size();
        for cell_y in (0..height).step_by(8) {
            for cell_x in (0..width).step_by(8) {
                let mut shades = [[None; 8]; 8];
                for (y, shade_row) in shades.iter_mut().enumerate() {
                    for (x, shade) in shade_row.iter_mut().enumerate() {
                        *shade = view.get_pixel(x + cell_x, y + cell_y);
                    }
                }

                let (char, palette) = shades_to_chardata(shades)?;
                let (index, hflip, vflip) = chardata.add_deduped(char);
                cells.push(
                    Cell::new()
                        .with_character(index)
                        .with_hflip(hflip)
                        .with_vflip(vflip)
                        .with_palette(palette)
                        .into_bits(),
                );
            }
        }

        self.imagedata.insert(
            name.to_string(),
            ImageData {
                name: name.to_string(),
                width,
                height,
                cells,
            },
        );
        Ok(())
    }

    fn process_mask(&mut self, name: String, mask: RawMask) -> Result<()> {
        let png = self.pngs.open(mask.region.file.to_path_buf())?;
        let ImageRegion {
            position,
            size,
            transform,
        } = parse_region(png, &mask.region)?;
        let view = png.view(position, size, transform);

        let mut pixels = vec![];
        let (width, height) = view.size();
        for y in 0..height {
            for cell_x in (0..width).step_by(8) {
                let mut collision_data = 0u8;
                for x in 0..8 {
                    collision_data >>= 1;
                    let pixel = view.get_pixel(x + cell_x, y);
                    if pixel.is_some_and(|s| s != Shade::Transparent) {
                        collision_data |= 0x80;
                    }
                }
                pixels.push(collision_data);
            }
        }

        self.maskdata.insert(
            name.to_string(),
            MaskData {
                name: name.to_string(),
                width,
                height,
                pixels,
            },
        );

        Ok(())
    }

    fn process_font(&mut self, name: String, font: RawFont) -> Result<()> {
        let ttf = self.ttfs.open(font.file.to_path_buf())?;
        let mut chars = vec![];
        for byte in 0u8..128u8 {
            let character = byte as char;
            chars.push(ttf.rasterize(character, font.size));
        }

        let width = chars.iter().map(|c| c.width).sum::<usize>();
        let baseline = chars.iter().map(|c| c.height).max().unwrap() as i32;
        let min_offset = chars.iter().map(|c| c.offset).min().unwrap();
        let height = (baseline - min_offset) as usize;

        let mut pixel_data = vec![0u8; width * height];
        let mut font_chars = Vec::with_capacity(chars.len());
        let mut current_x = 0;
        for char in chars {
            let y_offset = (baseline - char.offset) as usize - char.height;
            for y in 0..char.height {
                let src_start = y * char.width;
                let src_row = &char.pixels[src_start..src_start + char.width];
                let dst_start = (y_offset + y) * width + current_x;
                let dst_row = &mut pixel_data[dst_start..dst_start + char.width];
                for (dst, src) in dst_row.iter_mut().zip(src_row) {
                    *dst = match src {
                        Shade::Shade1 => 1,
                        Shade::Shade2 => 2,
                        Shade::Shade3 => 3,
                        _ => 0,
                    };
                }
            }
            font_chars.push(FontCharacterData {
                x: current_x as u16,
                y: 0,
                width: char.width as u16,
                height: (baseline - char.offset) as u16,
            });
            current_x += char.width;
        }

        let mut pixels = Vec::with_capacity(width.div_ceil(4) * height);
        for y in 0..height {
            let src_start = y * width;
            let src_row = &pixel_data[src_start..src_start + width];
            pixels.extend(src_row.chunks(4).map(|chunk| {
                let mut value = 0;
                for (i, pixel) in chunk.iter().enumerate() {
                    value |= pixel << (i * 2);
                }
                value
            }));
        }
        let texture_name = format!("{name}-data");
        self.texturedata.insert(
            texture_name.clone(),
            TextureData {
                name: texture_name.clone(),
                width,
                height,
                pixels,
            },
        );
        self.fontdata.insert(
            name.clone(),
            FontData {
                name,
                texture_name,
                line_height: height as u16,
                chars: font_chars,
            },
        );

        Ok(())
    }
}

fn parse_region(png: &PngContents, region: &RawImageRegion) -> Result<ImageRegion> {
    let position = region.position.unwrap_or_default();
    let size = region.size.unwrap_or(png.size);
    let mut transform = Transform {
        h_flip: region.hflip,
        v_flip: region.vflip,
        transpose: region.transpose,
        scale: region.scale,
    };
    match region.rotate % 360 {
        0 => {}
        90 => {
            transform.transpose = !transform.transpose;
            transform.h_flip = !transform.h_flip;
        }
        180 => {
            transform.h_flip = !transform.h_flip;
            transform.v_flip = !transform.v_flip;
        }
        270 => {
            transform.transpose = !transform.transpose;
            transform.v_flip = !transform.v_flip;
        }
        _ => bail!("Can only rotate multiples of 90 degrees"),
    }

    Ok(ImageRegion {
        position,
        size,
        transform,
    })
}

struct ImageRegion {
    position: (isize, isize),
    size: (usize, usize),
    transform: Transform,
}

fn shades_to_chardata(shades: [[Option<Shade>; 8]; 8]) -> Result<([u16; 8], u8)> {
    let mut char = [0; 8];
    let mut seen_shades = vec![];
    for shade in shades.iter().flatten().filter_map(|x| *x) {
        if !seen_shades.contains(&shade) {
            seen_shades.push(shade);
        }
    }

    let black_shade = if !seen_shades.contains(&Shade::Black) {
        0
    } else if !seen_shades.contains(&Shade::Shade1) {
        1
    } else if !seen_shades.contains(&Shade::Shade2) {
        2
    } else if !seen_shades.contains(&Shade::Shade3) {
        3
    } else {
        bail!("Too many shades in a single tile")
    };

    for (dst_row, src_row) in char.iter_mut().zip(shades) {
        for (x, src) in src_row.iter().enumerate() {
            let new_value = match src {
                Some(Shade::Transparent) | None => 0,
                Some(Shade::Black) => black_shade,
                Some(Shade::Shade1) => 1,
                Some(Shade::Shade2) => 2,
                Some(Shade::Shade3) => 3,
            };
            *dst_row |= new_value << (x * 2);
        }
    }
    Ok((char, black_shade as u8))
}

pub struct Assets {
    pub chardata: Vec<CharData>,
    pub images: Vec<ImageData>,
    pub masks: Vec<MaskData>,
    pub textures: Vec<TextureData>,
    pub fonts: Vec<FontData>,
}

pub struct CharData {
    pub name: String,
    pub chars: Vec<[u16; 8]>,
}
impl CharData {
    fn add_deduped(&mut self, char: [u16; 8]) -> (u16, bool, bool) {
        for v_flip in [false, true] {
            for h_flip in [false, true] {
                let transformed_char = flip_char(char, h_flip, v_flip);
                if let Some(index) = self.chars.iter().position(|c| c == &transformed_char) {
                    return (index as u16, h_flip, v_flip);
                }
            }
        }
        let index = self.chars.len();
        self.chars.push(char);
        (index as u16, false, false)
    }
}

pub struct ImageData {
    pub name: String,
    pub width: usize,
    pub height: usize,
    pub cells: Vec<u16>,
}

pub struct MaskData {
    pub name: String,
    pub width: usize,
    pub height: usize,
    pub pixels: Vec<u8>,
}

pub struct TextureData {
    pub name: String,
    pub width: usize,
    pub height: usize,
    pub pixels: Vec<u8>,
}

pub struct FontData {
    pub name: String,
    pub texture_name: String,
    pub line_height: u16,
    pub chars: Vec<FontCharacterData>,
}
pub struct FontCharacterData {
    pub x: u16,
    pub y: u16,
    pub width: u16,
    pub height: u16,
}
impl FontCharacterData {
    pub fn as_bytes(&self) -> [u8; 8] {
        let mut result = [0; 8];
        result[0..2].copy_from_slice(&self.x.to_le_bytes());
        result[2..4].copy_from_slice(&self.y.to_le_bytes());
        result[4..6].copy_from_slice(&self.width.to_le_bytes());
        result[6..8].copy_from_slice(&self.height.to_le_bytes());
        result
    }
}

fn flip_char(char: [u16; 8], h_flip: bool, v_flip: bool) -> [u16; 8] {
    let mut result = char;
    if v_flip {
        result.reverse();
    }
    if h_flip {
        for row in &mut result {
            // Iterative bit reverse idiom, but skip the final step.
            // Results in us swapping every pair of bits.
            *row = (*row & 0xff00) >> 8 | (*row & 0x00ff) << 8;
            *row = (*row & 0xf0f0) >> 4 | (*row & 0x0f0f) << 4;
            *row = (*row & 0xcccc) >> 2 | (*row & 0x3333) << 2;
        }
    }
    result
}

struct Transform {
    h_flip: bool,
    v_flip: bool,
    transpose: bool,
    scale: f64,
}

#[bitfield(u16)]
struct Cell {
    #[bits(11)]
    pub character: u16,
    _pad: bool,
    pub vflip: bool,
    pub hflip: bool,
    #[bits(2)]
    pub palette: u8,
}

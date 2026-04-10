mod font;
mod png;

use std::collections::{BTreeMap, BTreeSet};

use crate::{
    assets::{font::FontAtlas, png::PngContents},
    config::{
        RawAnimation, RawAssets, RawBgSprite, RawBgSpriteMap, RawFont, RawImage, RawImageRegion,
        RawMask,
    },
};
use anyhow::{Result, bail};
use bitfield_struct::bitfield;
use png::PngAtlas;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Shade {
    Transparent,
    Shade1,
    Shade2,
    Shade3,
    Black,
}

pub fn process(assets: RawAssets) -> Result<Assets> {
    AssetProcessor::new().process(assets)
}

struct AssetProcessor {
    pngs: PngAtlas,
    fonts: FontAtlas,
    chardata: BTreeMap<String, CharData>,
    animationdata: BTreeMap<String, AnimationData>,
    imagedata: BTreeMap<String, ImageData>,
    bgspritemapdata: BTreeMap<String, BgSpriteMapData>,
    maskdata: BTreeMap<String, MaskData>,
    texturedata: BTreeMap<String, TextureData>,
    fontdata: BTreeMap<String, FontData>,
}

impl AssetProcessor {
    pub fn new() -> Self {
        Self {
            pngs: PngAtlas::new(),
            fonts: FontAtlas::new(),
            chardata: BTreeMap::new(),
            animationdata: BTreeMap::new(),
            imagedata: BTreeMap::new(),
            bgspritemapdata: BTreeMap::new(),
            maskdata: BTreeMap::new(),
            texturedata: BTreeMap::new(),
            fontdata: BTreeMap::new(),
        }
    }

    pub fn process(mut self, mut assets: RawAssets) -> Result<Assets> {
        for (name, image) in assets.images {
            self.process_image(name, image)?;
        }
        for (name, animation) in assets.animations {
            self.process_animation(name, animation)?;
        }
        for (name, mask) in assets.masks {
            self.process_mask(name, mask)?;
        }
        for (name, font) in assets.fonts {
            self.process_font(name, font)?;
        }
        while let Some((name, sprite_map)) = assets.bg_sprite_maps.pop_first() {
            let mut current_base = sprite_map.base.clone();
            let mut sprite_map_queue = vec![];
            while let Some(base) = current_base.take() {
                if let Some(base_map) = assets.bg_sprite_maps.remove(&base) {
                    current_base = base_map.base.clone();
                    sprite_map_queue.push((base, base_map));
                }
            }
            for (name, sprite_map) in sprite_map_queue.into_iter().rev() {
                self.process_bg_sprite_map(name, sprite_map)?;
            }
            self.process_bg_sprite_map(name, sprite_map)?;
        }
        Ok(Assets {
            chardata: self.chardata.into_values().collect(),
            images: self.imagedata.into_values().collect(),
            animations: self.animationdata.into_values().collect(),
            bg_sprite_maps: self.bgspritemapdata.into_values().collect(),
            masks: self.maskdata.into_values().collect(),
            textures: self.texturedata.into_values().collect(),
            fonts: self.fontdata.into_values().collect(),
        })
    }

    fn process_image(&mut self, name: String, image: RawImage) -> Result<()> {
        let chardata = image.chardata.clone();
        let frame = self.extract_image(image)?;
        self.imagedata.insert(
            name.clone(),
            ImageData {
                name,
                width: frame.width,
                height: frame.height,
                cells: frame.cells,
                chardata,
            },
        );
        Ok(())
    }

    fn process_animation(&mut self, name: String, animation: RawAnimation) -> Result<()> {
        let mut frames = vec![];
        for image in animation.images {
            frames.push(self.extract_image(image)?);
        }
        self.animationdata.insert(
            name.clone(),
            AnimationData {
                name,
                chardata: animation.chardata,
                frames,
            },
        );
        Ok(())
    }

    fn extract_image(&mut self, image: RawImage) -> Result<FrameData> {
        let png = self.pngs.open(image.region.file.to_path_buf())?;
        let ImageRegion {
            position,
            size,
            transform,
        } = parse_region(png, &image.region)?;
        let view = png.view(position, image.palette, size, transform);

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
                let mut shades = [[Shade::Transparent; 8]; 8];
                for (y, shade_row) in shades.iter_mut().enumerate() {
                    for (x, shade) in shade_row.iter_mut().enumerate() {
                        *shade = view.get_shade(x + cell_x, y + cell_y);
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

        Ok(FrameData {
            width,
            height,
            cells,
        })
    }

    fn process_bg_sprite_map(&mut self, name: String, raw: RawBgSpriteMap) -> Result<()> {
        let (mut bgmap, mut x, mut y, mut row_height) = if let Some(base_name) = raw.base {
            let Some(base) = self.bgspritemapdata.get(&base_name) else {
                bail!("sprite map \"{name}\" has nonexistent base \"{base_name}\"");
            };
            (
                base.next_bgmap,
                base.next_x,
                base.next_y,
                base.next_row_height,
            )
        } else {
            (raw.bgmap_start, 0, 0, 0)
        };
        let mut sprites = vec![];
        let mut chardatas = BTreeSet::new();
        for (name, sprite) in raw.sprites {
            let (kind, image) = match sprite {
                RawBgSprite::Region { size, frames: None } => (
                    BgSpriteKind::Image(BgSpriteImageData {
                        width: size.0,
                        height: size.1,
                    }),
                    None,
                ),
                RawBgSprite::Region {
                    size,
                    frames: Some(frames),
                } => {
                    let (columns, rows) = animation_layout(size, frames);
                    (
                        BgSpriteKind::Animation(BgSpriteAnimationData {
                            frame_width: size.0,
                            frame_height: size.1,
                            columns,
                            rows,
                        }),
                        None,
                    )
                }
                RawBgSprite::Image { image } => {
                    if let Some(data) = self.imagedata.get(&image) {
                        (
                            BgSpriteKind::Image(BgSpriteImageData {
                                width: data.width,
                                height: data.height,
                            }),
                            Some(ImageRefData {
                                name: image,
                                chardata: data.chardata.clone(),
                            }),
                        )
                    } else if let Some(data) = self.animationdata.get(&image) {
                        let &FrameData {
                            width: frame_width,
                            height: frame_height,
                            ..
                        } = &data.frames[0];
                        for frame in &data.frames {
                            if frame.width != frame_width || frame.height != frame_height {
                                bail!("all frames of animation \"{image}\" must be the same size");
                            }
                        }
                        let (columns, rows) =
                            animation_layout((frame_width, frame_height), data.frames.len());
                        (
                            BgSpriteKind::Animation(BgSpriteAnimationData {
                                frame_width,
                                frame_height,
                                columns,
                                rows,
                            }),
                            Some(ImageRefData {
                                name: image,
                                chardata: data.chardata.clone(),
                            }),
                        )
                    } else {
                        bail!("unrecognized image \"{image}\" in bgspritemap \"{name}\"");
                    }
                }
            };
            if let Some(image) = &image {
                chardatas.insert(image.chardata.clone());
            }
            let (width, height) = match &kind {
                BgSpriteKind::Image(data) => (data.width, data.height),
                BgSpriteKind::Animation(data) => (
                    data.frame_width * data.columns,
                    data.frame_height * data.rows,
                ),
            };
            if x + width > 512 {
                x = 0;
                y += row_height;
                row_height = height;
            } else {
                row_height = row_height.max(height);
            }
            if y + row_height > 512 {
                bgmap += 1;
                x = 0;
                y = 0;
                row_height = height;
            }
            sprites.push(BgSpriteData {
                name,
                bgmap,
                x,
                y,
                kind,
                image,
            });
            x += width;
        }

        self.bgspritemapdata.insert(
            name.clone(),
            BgSpriteMapData {
                name,
                sprites,
                chardatas: chardatas.into_iter().collect(),
                next_bgmap: bgmap,
                next_x: x,
                next_y: y,
                next_row_height: row_height,
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
        let view = png.view(position, None, size, transform);

        let mut pixels = vec![];
        let (width, height) = view.size();
        for y in 0..height {
            for cell_x in (0..width).step_by(8) {
                let mut collision_data = 0u8;
                for x in 0..8 {
                    collision_data >>= 1;
                    let shade = view.get_shade(x + cell_x, y);
                    if shade != Shade::Transparent {
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
        let contents = self.fonts.open(font.file.to_path_buf())?;
        let mut chars = vec![];
        for byte in 0u8..128u8 {
            let character = byte as char;
            chars.push(contents.rasterize(character, font.size));
        }

        let width = chars.iter().map(|c| c.width).sum::<usize>() + chars.len();
        let height = chars.iter().map(|c| c.height).max().unwrap();
        let baseline = chars
            .iter()
            .map(|c| (c.height as i32) + c.offset)
            .max()
            .unwrap();

        let mut pixel_data = vec![0u8; width * height];
        let mut font_chars = Vec::with_capacity(chars.len());
        let mut current_x = 0;
        for char in chars {
            let y_offset = (baseline - char.offset) as usize - char.height;
            for y in 0..char.height {
                let src_start = y * char.width;
                let src_row = &char.pixels[src_start..src_start + char.width];
                let dst_start = y * width + current_x;
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
                y_offset: y_offset as u16,
                width: char.width as u16,
                height: char.height as u16,
            });
            current_x += char.width + 1;
        }
        let line_height = font_chars
            .iter()
            .map(|c| c.y_offset + c.height)
            .max()
            .unwrap();

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
                line_height,
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

fn shades_to_chardata(shades: [[Shade; 8]; 8]) -> Result<([u16; 8], u8)> {
    let mut char = [0; 8];
    let mut seen_shades = vec![];
    for shade in shades.iter().copied().flatten() {
        if !seen_shades.contains(&shade) {
            seen_shades.push(shade);
        }
    }
    if seen_shades.len() == 5 {
        bail!("Too many shades in a single tile");
    }

    let black_shade = if !seen_shades.contains(&Shade::Shade1) {
        1
    } else if !seen_shades.contains(&Shade::Shade2) {
        2
    } else if !seen_shades.contains(&Shade::Shade3) {
        3
    } else {
        0
    };

    for (dst_row, src_row) in char.iter_mut().zip(shades) {
        for (x, src) in src_row.iter().enumerate() {
            let new_value = match src {
                Shade::Transparent => 0,
                Shade::Black => black_shade,
                Shade::Shade1 => 1,
                Shade::Shade2 => 2,
                Shade::Shade3 => 3,
            };
            *dst_row |= new_value << (x * 2);
        }
    }
    Ok((char, black_shade as u8))
}

pub struct Assets {
    pub chardata: Vec<CharData>,
    pub images: Vec<ImageData>,
    pub animations: Vec<AnimationData>,
    pub bg_sprite_maps: Vec<BgSpriteMapData>,
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

pub struct AnimationData {
    pub name: String,
    chardata: String,
    pub frames: Vec<FrameData>,
}

pub struct FrameData {
    pub width: usize,
    pub height: usize,
    pub cells: Vec<u16>,
}

pub struct ImageData {
    pub name: String,
    pub width: usize,
    pub height: usize,
    pub cells: Vec<u16>,
    chardata: String,
}

#[derive(Debug)]
pub struct BgSpriteMapData {
    pub name: String,
    pub sprites: Vec<BgSpriteData>,
    pub chardatas: Vec<String>,
    next_bgmap: u8,
    next_x: usize,
    next_y: usize,
    next_row_height: usize,
}

#[derive(Debug)]
pub struct BgSpriteData {
    pub name: String,
    pub bgmap: u8,
    pub x: usize,
    pub y: usize,
    pub kind: BgSpriteKind,
    pub image: Option<ImageRefData>,
}

#[derive(Debug)]
pub enum BgSpriteKind {
    Image(BgSpriteImageData),
    Animation(BgSpriteAnimationData),
}

#[derive(Debug)]
pub struct BgSpriteImageData {
    pub width: usize,
    pub height: usize,
}

#[derive(Debug)]
pub struct BgSpriteAnimationData {
    pub frame_width: usize,
    pub frame_height: usize,
    pub columns: usize,
    pub rows: usize,
}

#[derive(Debug)]
pub struct ImageRefData {
    pub name: String,
    pub chardata: String,
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
    pub y_offset: u16,
    pub width: u16,
    pub height: u16,
}
impl FontCharacterData {
    pub fn as_bytes(&self) -> [u8; 8] {
        let mut result = [0; 8];
        result[0..2].copy_from_slice(&self.x.to_le_bytes());
        result[2..4].copy_from_slice(&self.y_offset.to_le_bytes());
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

// minimize area, but then go for "square" shapes (where width is close to height)
fn animation_layout(frame_size: (usize, usize), frames: usize) -> (usize, usize) {
    let mut result = (frames, 1);
    let mut area = frames * frame_size.0 * frame_size.1;
    let mut squareness = (frames * frame_size.0).abs_diff(frame_size.1);
    for columns in (1..frames).rev() {
        let rows = frames.div_ceil(columns);
        let width = columns * frame_size.0;
        let height = rows * frame_size.1;
        let new_area = width * height;
        let new_squareness = width.abs_diff(height);
        if new_area
            .cmp(&area)
            .then(new_squareness.cmp(&squareness))
            .is_lt()
        {
            result = (columns, rows);
            area = new_area;
            squareness = new_squareness;
        }
    }
    result
}

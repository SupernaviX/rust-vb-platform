mod png;

use std::collections::BTreeMap;

use crate::{
    Options,
    config::{RawAssets, RawImage},
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

pub fn process(opts: &mut Options, assets: RawAssets) -> Result<Assets> {
    ImageProcessor::new(opts).process(assets)
}

struct ImageProcessor<'a> {
    opts: &'a mut Options,
    pngs: PngAtlas,
    chardata: BTreeMap<String, CharData>,
    imagedata: BTreeMap<String, ImageData>,
}

impl<'a> ImageProcessor<'a> {
    pub fn new(opts: &'a mut Options) -> Self {
        Self {
            opts,
            pngs: PngAtlas::new(),
            chardata: BTreeMap::new(),
            imagedata: BTreeMap::new(),
        }
    }

    pub fn process(mut self, assets: RawAssets) -> Result<Assets> {
        for (name, image) in assets.images {
            self.process_image(name, image)?;
        }
        Ok(Assets {
            chardata: self.chardata.into_values().collect(),
            images: self.imagedata.into_values().collect(),
        })
    }

    fn process_image(&mut self, name: String, image: RawImage) -> Result<()> {
        let png = self.pngs.open(self.opts.input_path(&image.file))?;

        let position = image.position.unwrap_or_default();
        let size = image
            .size
            .unwrap_or((png.size.0 - position.0, png.size.1 - position.1));
        let mut transform = Transform::default();
        if image.hflip {
            transform.h_flip = true;
        }
        if image.vflip {
            transform.v_flip = true;
        }
        if image.transpose {
            transform.transpose = true;
        }
        match image.rotate % 360 {
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
        let view = png.view(position, size, transform);

        let chardata = self
            .chardata
            .entry(image.chardata)
            .or_insert_with_key(|name| CharData {
                name: name.clone(),
                chars: vec![[0; 8]],
            });

        let mut cells = vec![];
        for cell_y in (0..size.1).step_by(8) {
            for cell_x in (0..size.0).step_by(8) {
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
                width: size.0,
                height: size.1,
                cells,
            },
        );
        Ok(())
    }
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

#[derive(Default)]
struct Transform {
    h_flip: bool,
    v_flip: bool,
    transpose: bool,
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

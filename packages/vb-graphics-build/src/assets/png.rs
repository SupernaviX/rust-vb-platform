use anyhow::{Result, bail};
use png::{ColorType, Decoder, Transformations};
use std::{
    collections::{BTreeMap, btree_map::Entry},
    fs::File,
    path::{Path, PathBuf},
};

use super::{Shade, Transform};

pub struct PngAtlas {
    files: BTreeMap<PathBuf, PngContents>,
}

impl PngAtlas {
    pub fn new() -> Self {
        Self {
            files: BTreeMap::new(),
        }
    }

    pub fn open(&mut self, full_path: PathBuf) -> Result<&PngContents> {
        match self.files.entry(full_path) {
            Entry::Occupied(e) => Ok(e.into_mut()),
            Entry::Vacant(e) => {
                let contents = load_png_contents(e.key())?;
                Ok(e.insert(contents))
            }
        }
    }
}

pub struct PngContents {
    pixels: Vec<Shade>,
    pub size: (usize, usize),
}

impl PngContents {
    fn from_color_alpha(bytes: &[u8], size: (usize, usize)) -> Result<Self> {
        let new_bytes: Vec<u8> = array_chunks(bytes)
            .flat_map(|[r, _, _, a]| vec![*r, *a])
            .collect();
        Self::from_greyscale_alpha(&new_bytes, size)
    }
    fn from_greyscale_alpha(bytes: &[u8], size: (usize, usize)) -> Result<Self> {
        let palette_lookup = [Shade::Black, Shade::Shade1, Shade::Shade2, Shade::Shade3];

        let pixels = array_chunks(bytes)
            .map(|[shade, alpha]| {
                if *alpha == 0 {
                    Shade::Transparent
                } else {
                    palette_lookup[*shade as usize / 64]
                }
            })
            .collect();
        Ok(Self { pixels, size })
    }
    pub fn get_pixel(&self, x: usize, y: usize) -> Option<Shade> {
        if x >= self.size.0 || y >= self.size.1 {
            return None;
        }
        Some(self.pixels[y * self.size.0 + x])
    }
    pub fn view(
        &self,
        position: (usize, usize),
        size: (usize, usize),
        transform: Transform,
    ) -> PngView {
        PngView {
            png: self,
            position,
            size,
            transform,
        }
    }
}

fn load_png_contents(path: &Path) -> Result<PngContents> {
    let file = File::open(path)?;
    let mut decoder = Decoder::new(file);
    decoder.set_transformations(Transformations::normalize_to_color8() | Transformations::ALPHA);
    let mut reader = decoder.read_info()?;

    let mut buf = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buf)?;
    buf.truncate(info.buffer_size());

    let size = (info.width as usize, info.height as usize);

    match info.color_type {
        ColorType::GrayscaleAlpha => PngContents::from_greyscale_alpha(&buf, size),
        ColorType::Rgba => PngContents::from_color_alpha(&buf, size),
        _ => bail!("Unexpected color type {:?}", info.color_type),
    }
}

pub struct PngView<'a> {
    png: &'a PngContents,
    position: (usize, usize),
    size: (usize, usize),
    transform: Transform,
}

impl PngView<'_> {
    pub fn get_pixel(&self, x: usize, y: usize) -> Option<Shade> {
        if x >= self.size.0 || y >= self.size.1 {
            return None;
        }
        let (mut rel_x, mut rel_y) = (x, y);
        if self.transform.h_flip {
            rel_x = self.size.0 - rel_x;
        }
        if self.transform.v_flip {
            rel_y = self.size.1 - rel_y;
        }
        if self.transform.transpose {
            std::mem::swap(&mut rel_x, &mut rel_y);
        }
        let real_x = self.position.0 + rel_x;
        let real_y = self.position.1 + rel_y;
        self.png.get_pixel(real_x, real_y)
    }
}

// This is here because the real array_chunks is still unstable
fn array_chunks<T, const N: usize>(slice: &[T]) -> impl Iterator<Item = &[T; N]> {
    ArrayChunks { slice }
}

struct ArrayChunks<'a, T, const N: usize> {
    slice: &'a [T],
}
impl<'a, T, const N: usize> Iterator for ArrayChunks<'a, T, N> {
    type Item = &'a [T; N];

    fn next(&mut self) -> Option<Self::Item> {
        if self.slice.len() < N {
            return None;
        }
        let (next, rest) = self.slice.split_at(N);
        self.slice = rest;
        next.try_into().ok()
    }
}

use vb_rt::sys::vip;

pub struct Image {
    pub width_cells: u8,
    pub height_cells: u8,
    pub data: &'static [vip::Cell],
}

impl Image {
    pub fn render_to_bgmap(&self, index: u8, dst: (u8, u8)) -> (i16, i16) {
        self.render_region_to_bgmap(index, dst, (0, 0), (self.width_cells, self.height_cells))
    }

    pub fn render_region_to_bgmap(
        &self,
        index: u8,
        dst: (u8, u8),
        src: (u8, u8),
        cells: (u8, u8),
    ) -> (i16, i16) {
        let map = vip::BG_MAPS.index(index as usize);
        for y in 0..cells.1 {
            let src_y = (src.1 + y) as usize;
            let src_start = src_y * self.width_cells as usize + src.0 as usize;
            let src_end = src_start + cells.0 as usize;
            let src_data = &self.data[src_start..src_end];

            let dst_y = (dst.1 + y) as usize;
            let dst_start = dst_y * 64 + dst.0 as usize;
            map.write_slice(src_data, dst_start);
        }
        (dst.0 as i16 * 8, dst.1 as i16 * 8)
    }

    pub fn render_to_objects(&self, end: u16, dst: (i16, i16), stereo: vip::ObjectStereo) -> u16 {
        let mut index = end;
        for y in 0..self.height_cells {
            let dy = dst.1 + (y as i16) * 8;
            if dy <= -8 {
                continue;
            }
            if dy >= 224 {
                break;
            }
            for x in 0..self.width_cells {
                let dx = dst.0 + (x as i16) * 8;
                if dx <= -8 {
                    continue;
                }
                if dx >= 384 {
                    break;
                }
                let cell = self.data[y as usize * self.width_cells as usize + x as usize];
                if cell.character() == 0 {
                    continue;
                }
                let obj = vip::OBJS.index(index as usize);
                obj.jx().write(dx);
                obj.stereo().write(stereo);
                obj.jy().write(dy);
                obj.cell().write(cell);
                index -= 1;
            }
        }
        index
    }
}

#[derive(Debug)]
pub struct Mask {
    pub width: u16,
    pub height: u16,
    pub data: &'static [u8],
}

impl Mask {
    pub fn intersects(&self, other: &Mask, offset: (i16, i16)) -> bool {
        let left = offset.0.max(0) as usize;
        let right = other.width.saturating_add_signed(offset.0).min(self.width) as usize;
        let top = offset.1.max(0) as usize;
        let bottom = other
            .height
            .saturating_add_signed(offset.1)
            .min(self.height) as usize;

        let lhs_width_cells = self.width.div_ceil(8) as usize;
        let rhs_width_cells = other.width.div_ceil(8) as usize;
        let length = right - left;
        for y in top..bottom {
            let other_left = left.saturating_add_signed(-offset.0 as isize);
            let other_y = y.saturating_add_signed(-offset.1 as isize);

            let lhs_bytes = &self.data[(y * lhs_width_cells + left / 8)..];
            let lhs_offset = left % 8;
            let rhs_bytes = &other.data[(other_y * rhs_width_cells + other_left / 8)..];
            let rhs_offset = other_left % 8;
            if Self::row_intersects(lhs_bytes, lhs_offset, rhs_bytes, rhs_offset, length) {
                return true;
            }
        }
        false
    }

    fn row_intersects(
        mut lhs_bytes: &[u8],
        mut lhs_offset: usize,
        mut rhs_bytes: &[u8],
        mut rhs_offset: usize,
        mut length: usize,
    ) -> bool {
        while length > 0 {
            let to_consume = length.min(8 - lhs_offset).min(8 - rhs_offset);
            let lhs = lhs_bytes[0] << to_consume;
            let rhs = rhs_bytes[0] << to_consume;
            if lhs & rhs != 0 {
                return true;
            }
            lhs_offset += to_consume;
            if lhs_offset == 8 {
                lhs_offset = 0;
                lhs_bytes = lhs_bytes.split_at(1).1;
            }
            rhs_offset += to_consume;
            if rhs_offset == 8 {
                rhs_offset = 0;
                rhs_bytes = rhs_bytes.split_at(1).1;
            }
            length -= to_consume;
        }
        false
    }
}

#[derive(Debug)]
pub struct Texture {
    pub width: u16,
    pub height: u16,
    pub data: &'static [u8],
}

impl Texture {
    pub fn render_row_to_chardata(&self, index: u16, dst: (u8, u8), src: (u16, u16), size: u16) {
        let mut dst_addr = (index as usize * 8) + dst.1 as usize;
        let src_addr = self.width.div_ceil(4) as usize * src.1 as usize + (src.0 as usize / 4);
        let src_offset = (src.0 % 4) as u8;
        let dst_offset = dst.0;

        let row_iter = TextureRowIter {
            data: &self.data[src_addr..],
            remaining: size as usize,
            src_offset,
            dst_offset,
        };

        for TextureCell { data, mask } in row_iter {
            let dst = vip::CHARACTER_HWS.index(dst_addr);
            let value = if mask != 0 {
                (dst.read() & mask) | data
            } else {
                data
            };
            dst.write(value);
            dst_addr += 8;
        }
    }
}

struct TextureRowIter<'a> {
    data: &'a [u8],
    remaining: usize,
    src_offset: u8,
    dst_offset: u8,
}

impl<'a> Iterator for TextureRowIter<'a> {
    type Item = TextureCell;

    fn next(&mut self) -> Option<Self::Item> {
        if self.remaining == 0 {
            return None;
        }
        let mut data = 0x0000u16;
        let mut mask = 0xffffu16;

        let mut pixels_needed = self.remaining.min(8 - self.dst_offset as usize);
        let mut pixels_used = self.dst_offset;
        self.dst_offset = 0;
        while pixels_needed > 0 {
            let pixel_count = (4 - self.src_offset as usize).min(pixels_needed);
            let pixels = self.data[0] >> (self.src_offset * 2);
            data = (data >> (pixel_count * 2)) | ((pixels as u16) << (16 - pixel_count * 2));
            mask >>= pixel_count * 2;
            pixels_needed -= pixel_count;
            pixels_used += pixel_count as u8;
            self.remaining -= pixel_count;
            self.src_offset += pixel_count as u8;
            if self.src_offset == 4 {
                self.data = &self.data[1..];
                self.src_offset = 0;
            }
        }

        let remaining_shift = 16 - pixels_used * 2;
        data >>= remaining_shift;
        mask >>= remaining_shift;

        Some(TextureCell { data, mask })
    }
}

struct TextureCell {
    data: u16,
    mask: u16,
}

#[repr(C)]
#[derive(Debug)]
pub struct FontCharacter {
    pub x: u16,
    pub y: u16,
    pub width: u16,
    pub height: u16,
}

#[derive(Debug)]
pub struct Font {
    pub texture: &'static Texture,
    pub chars: &'static [FontCharacter],
    pub line_height: u16,
}

impl Font {
    pub fn measure(&self, text: &[u8]) -> u16 {
        let mut width = 0;
        for char in text {
            width += self.chars[*char as usize].width + 1;
        }
        width
    }
}

#[macro_export]
#[cfg(windows)]
macro_rules! path_sep {
    () => {
        "\\"
    };
}
#[macro_export]
#[cfg(not(windows))]
macro_rules! path_sep {
    () => {
        "/"
    };
}

#[macro_export]
macro_rules! out_path {
    ($filename:expr) => {
        concat!(env!("OUT_DIR"), $crate::path_sep!(), $filename)
    };
}

#[macro_export]
macro_rules! include_chardata {
    ($path:expr) => {
        $crate::resource_value_impl!(4, include_bytes!($crate::out_path!($path)))
    };
}

#[macro_export]
macro_rules! include_celldata {
    ($path:expr) => {
        $crate::resource_value_impl!(4, include_bytes!($crate::out_path!($path)))
    };
}

#[macro_export]
macro_rules! include_maskdata {
    ($path:expr) => {
        include_bytes!($crate::out_path!($path))
    };
}

#[macro_export]
macro_rules! include_texturedata {
    ($path:expr) => {
        include_bytes!($crate::out_path!($path))
    };
}

#[macro_export]
macro_rules! include_fontdata {
    ($path:expr) => {
        $crate::resource_value_impl!(4, include_bytes!($crate::out_path!($path)))
    };
}

#[macro_export]
macro_rules! resource_value_impl {
    ($align:expr, $contents:expr) => {{
        #[repr(C, align($align))]
        struct _Aligned<T>(T);

        const ALIGNED: _Aligned<[u8; $contents.len()]> = _Aligned(*$contents);
        unsafe { core::mem::transmute(ALIGNED.0) }
    }};
}

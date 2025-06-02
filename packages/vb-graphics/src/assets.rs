use vb_rt::sys::vip;

pub struct Image {
    pub width_cells: u8,
    pub height_cells: u8,
    pub data: &'static [vip::BGCell],
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
        mask = (mask >> remaining_shift) | !(u16::MAX >> remaining_shift);

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

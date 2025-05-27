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
macro_rules! resource_value_impl {
    ($align:expr, $contents:expr) => {{
        #[repr(C, align($align))]
        struct _Aligned<T>(T);

        const ALIGNED: _Aligned<[u8; $contents.len()]> = _Aligned(*$contents);
        unsafe { core::mem::transmute(ALIGNED.0) }
    }};
}

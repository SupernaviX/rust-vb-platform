use vb_rt::sys::vip;

pub struct Image {
    pub width_cells: u16,
    pub height_cells: u16,
    pub data: &'static [vip::BGCell],
}

impl Image {
    pub fn render_to_bgmap(&self, index: u16, x: u16, y: u16) {
        let map = vip::BG_MAPS.index(index as usize);
        let offsets = (y..y + self.height_cells)
            .flat_map(move |y| (x..x + self.width_cells).map(move |x| y * 64 + x));
        for (src, offset) in self.data.iter().zip(offsets) {
            let dst = map.index(offset as usize);
            dst.write(*src);
        }
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

#[repr(C, align(4))]
pub struct WaveformData<const N: usize>([u8; N]);
impl<const N: usize> WaveformData<N> {
    pub const fn as_slice(&self) -> &[u8] {
        self.0.as_slice()
    }
}
impl<const N: usize> core::ops::Deref for WaveformData<N> {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        &self.0
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
macro_rules! include_channel {
    ($path:expr) => {
        $crate::resource_value_impl!(4, include_bytes!($crate::out_path!($path)))
    };
}

#[macro_export]
macro_rules! include_waveforms {
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

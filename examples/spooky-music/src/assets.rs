include!(concat!(env!("OUT_DIR"), "/assets.rs"));

pub static CHANNEL_0: [u32; 449] =
    vb_graphics::resource_value_impl!(4, include_bytes!("../assets/channel_0.bin"));
pub static CHANNEL_1: [u32; 4484] =
    vb_graphics::resource_value_impl!(4, include_bytes!("../assets/channel_1.bin"));
pub static CHANNEL_2: [u32; 598] =
    vb_graphics::resource_value_impl!(4, include_bytes!("../assets/channel_2.bin"));
pub static CHANNEL_3: [u32; 1781] =
    vb_graphics::resource_value_impl!(4, include_bytes!("../assets/channel_3.bin"));

include!(concat!(env!("OUT_DIR"), "/assets.rs"));

pub const CHANNEL_0: [u32; 385] =
    vb_graphics::resource_value_impl!(4, include_bytes!("../assets/channel_0.bin"));
pub const CHANNEL_1: [u32; 940] =
    vb_graphics::resource_value_impl!(4, include_bytes!("../assets/channel_1.bin"));
pub const CHANNEL_2: [u32; 124] =
    vb_graphics::resource_value_impl!(4, include_bytes!("../assets/channel_2.bin"));
#[allow(unused)]
pub const CHANNEL_3: [u32; 579] =
    vb_graphics::resource_value_impl!(4, include_bytes!("../assets/channel_3.bin"));

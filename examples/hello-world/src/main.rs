#![no_main]
#![no_std]

use vb_graphics as gfx;
use vb_rt::sys::{halt, vip};

vb_rt::rom_header!("Hello World!", "SG", "HIYA");
vb_rt::main!({ main() });

const CHARS: [vip::Character; 1] = [vip::Character([
    0xfa50, 0xfa50, 0xfa50, 0xfa50, 0xfa50, 0xfa50, 0xfa50, 0xfa50,
])];

fn main() {
    gfx::init_display();
    gfx::set_colors(32, 64, 32);
    gfx::set_bkcol(0);
    gfx::load_character_data(&CHARS, 1);

    vip::GPLT0.write(vip::Palette::new().with_c1(1).with_c2(2).with_c3(3));

    vip::BG_CELLS
        .index(0)
        .write(vip::BGCell::new().with_character(1));
    vip::BG_CELLS
        .index(1)
        .write(vip::BGCell::new().with_character(1).with_bhflp(true));

    let world = vip::WORLDS.index(31);
    world.header().write(
        vip::WorldHeader::new()
            .with_lon(true)
            .with_ron(true)
            .with_bgm(vip::WorldMode::Normal)
            .with_bg_map_base(0),
    );
    world.w().write(15);
    world.h().write(7);
    loop {
        halt();
    }
}

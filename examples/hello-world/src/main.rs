#![no_main]
#![no_std]

use vb_graphics as gfx;
use vb_rt::sys::{halt, vip::Character};

vb_rt::rom_header!("Hello World!", "SG", "HIYA");
vb_rt::main!({ main() });

const CHARS: [Character; 1] = [Character([
    0xfa50, 0xfa50, 0xfa50, 0xfa50, 0xfa50, 0xfa50, 0xfa50, 0xfa50,
])];

fn main() {
    gfx::init_display();
    gfx::set_colors(32, 64, 32);
    gfx::set_bkcol(2);
    gfx::load_character_data(&CHARS, 1);
    loop {
        halt();
    }
}

#![no_main]
#![no_std]

use vb_graphics as gfx;
use vb_rt::sys::halt;

vb_rt::rom_header!("Hello World!", "SG", "HIYA");
vb_rt::main!({ main() });

fn main() {
    gfx::init_display();
    gfx::set_colors(32, 64, 32);
    gfx::set_bkcol(2);
    loop {
        halt();
    }
}

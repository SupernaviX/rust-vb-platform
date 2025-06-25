#![no_main]
#![no_std]

mod assets;

use vb_graphics::{self as gfx, text::TextRenderer};
use vb_rt::sys::vip;

vb_rt::rom_header!("Display Text", "SG", "TEXT");
vb_rt::main!({ main() });

static FRAME: gfx::FrameMonitor = gfx::FrameMonitor::new();
vb_rt::vip_interrupt_handler!({
    FRAME.acknowledge_interrupts();
});

fn main() {
    gfx::init_display();
    gfx::set_colors(32, 64, 32);
    gfx::set_bkcol(0);

    const TEXT_WIDTH_CHARS: u8 = 34;
    const TEXT_HEIGHT_CHARS: u8 = 8;

    let mut renderer =
        TextRenderer::new(&assets::ALAGARD, 16, (TEXT_WIDTH_CHARS, TEXT_HEIGHT_CHARS));
    renderer.draw_text(b"I can render fonts from TTF files,\nbut it is uglier than I hoped...\nActually this font looks much nicer!");
    renderer.render_to_bgmap(0, (0, 0));

    FRAME.enable_interrupts();

    loop {
        let world = vip::WORLDS.index(31);
        world.header().write(
            vip::WorldHeader::new()
                .with_lon(true)
                .with_ron(true)
                .with_bgm(vip::WorldMode::Normal)
                .with_bg_map_base(0),
        );
        world.gx().write((384 - (TEXT_WIDTH_CHARS as i16 * 8)) / 2);
        world.gy().write((224 - (TEXT_HEIGHT_CHARS as i16 * 8)) / 2);
        world.w().write((TEXT_WIDTH_CHARS as i16 * 8) - 1);
        world.h().write((TEXT_HEIGHT_CHARS as i16 * 8) - 1);

        let world = vip::WORLDS.index(30);
        world.header().write(vip::WorldHeader::new().with_end(true));

        FRAME.wait_for_new_frame();
    }
}

#![no_main]
#![no_std]

mod assets;

use fixed::types::I10F6;
use vb_graphics as gfx;
use vb_rt::{
    println,
    sys::{hardware, vip},
};

vb_rt::rom_header!("Hello World!", "SG", "HIYA");
vb_rt::main!({ main() });

static FRAME: gfx::FrameMonitor = gfx::FrameMonitor::new();
vb_rt::vip_interrupt_handler!({
    FRAME.acknowledge_interrupts();
});

fn main() {
    gfx::init_display();
    gfx::set_colors(32, 64, 32);
    gfx::set_bkcol(0);
    gfx::load_character_data(&assets::ALL, 0);

    assets::BACKGROUND.render_to_bgmap(0, (0, 0));
    assets::SMILE.render_to_bgmap(0, (48, 0));

    FRAME.enable_interrupts();

    let mut counter = 0;
    let mut smile_x = I10F6::from_num(184);
    let mut smile_y = I10F6::from_num(104);

    loop {
        let world = vip::WORLDS.index(31);
        world.header().write(
            vip::WorldHeader::new()
                .with_lon(true)
                .with_ron(true)
                .with_bgm(vip::WorldMode::Normal)
                .with_bg_map_base(0),
        );
        world.w().write(383);
        world.h().write(223);

        let world = vip::WORLDS.index(30);
        world.header().write(
            vip::WorldHeader::new()
                .with_lon(true)
                .with_ron(true)
                .with_bgm(vip::WorldMode::Normal)
                .with_bg_map_base(0),
        );
        world.gx().write(smile_x.round().to_num());
        world.gy().write(smile_y.round().to_num());
        world.mx().write(384);
        world.my().write(0);
        world.w().write(15);
        world.h().write(15);

        let world = vip::WORLDS.index(29);
        world.header().write(vip::WorldHeader::new().with_end(true));

        let buttons = hardware::read_controller();
        let mut xspeed = I10F6::ZERO;
        let mut yspeed = I10F6::ZERO;
        if buttons.ll() {
            xspeed -= I10F6::from_num(2);
        }
        if buttons.lr() {
            xspeed += I10F6::from_num(2);
        }
        if buttons.lu() {
            yspeed -= I10F6::from_num(2);
        }
        if buttons.ld() {
            yspeed += I10F6::from_num(2);
        }
        if xspeed != 0 && yspeed != 0 {
            xspeed *= I10F6::SQRT_2 / 2;
            yspeed *= I10F6::SQRT_2 / 2;
        }
        println!("{counter}: ({xspeed:?}, {yspeed:?})");
        counter += 1;
        smile_x = (smile_x + xspeed).clamp(I10F6::ZERO, I10F6::from_num(368));
        smile_y = (smile_y + yspeed).clamp(I10F6::ZERO, I10F6::from_num(208));

        FRAME.wait_for_new_frame();
    }
}

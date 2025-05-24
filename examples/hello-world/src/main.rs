#![no_main]
#![no_std]

use vb_graphics::{self as gfx, Image};
use vb_rt::sys::{hardware, vip};

vb_rt::rom_header!("Hello World!", "SG", "HIYA");
vb_rt::main!({ main() });

static FRAME: gfx::FrameMonitor = gfx::FrameMonitor::new();
vb_rt::vip_interrupt_handler!({
    FRAME.acknowledge_interrupts();
});

const CHARS: [vip::Character; 4] = [
    // stripe tile 1
    vip::Character([
        0x5555, 0x9555, 0xa555, 0xa955, 0xaa55, 0xaa95, 0xaaa5, 0xaaa9,
    ]),
    // stripe tile 2
    vip::Character([
        0xaaaa, 0x6aaa, 0x5aaa, 0x56aa, 0x55aa, 0x556a, 0x555a, 0x5556,
    ]),
    // smile top left
    vip::Character([
        0x5500, 0xff40, 0xffe0, 0xfff4, 0xf7fd, 0xf7fd, 0xf7fd, 0xfffd,
    ]),
    // smile bottom left
    vip::Character([
        0xfffd, 0xfffd, 0xff7d, 0xfdfd, 0x57f4, 0xffd0, 0xff40, 0x5500,
    ]),
];

const BACKGROUND_DATA: [vip::BGCell; 48 * 28] = {
    let mut arr = [vip::BGCell::new(); 48 * 28];
    let mut i = 0;
    while i < arr.len() {
        let x = i % 48;
        let y = i / 48;
        let index = if x % 2 == y % 2 { 1 } else { 2 };
        arr[i] = vip::BGCell::new().with_character(index);
        i += 1;
    }
    arr
};

const BACKGROUND: Image = Image {
    width_cells: 48,
    height_cells: 28,
    data: &BACKGROUND_DATA,
};

const SMILE_DATA: [vip::BGCell; 2 * 2] = [
    vip::BGCell::new().with_character(3).with_gplts(1),
    vip::BGCell::new()
        .with_character(3)
        .with_gplts(1)
        .with_bhflp(true),
    vip::BGCell::new().with_character(4).with_gplts(1),
    vip::BGCell::new()
        .with_character(4)
        .with_gplts(1)
        .with_bhflp(true),
];

const SMILE: Image = Image {
    width_cells: 2,
    height_cells: 2,
    data: &SMILE_DATA,
};

fn main() {
    gfx::init_display();
    gfx::set_colors(32, 64, 32);
    gfx::set_bkcol(0);
    gfx::load_character_data(&CHARS, 1);

    vip::GPLT0.write(vip::Palette::new().with_c1(1).with_c2(2).with_c3(3));
    vip::GPLT1.write(vip::Palette::new().with_c1(0).with_c2(2).with_c3(3));

    BACKGROUND.render_to_bgmap(0, 0, 0);
    SMILE.render_to_bgmap(0, 48, 0);

    FRAME.enable_interrupts();

    let mut smile_x = 184;
    let mut smile_y = 104;

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
        world.gx().write(smile_x);
        world.gy().write(smile_y);
        world.mx().write(384);
        world.my().write(0);
        world.w().write(15);
        world.h().write(15);

        let world = vip::WORLDS.index(29);
        world.header().write(vip::WorldHeader::new().with_end(true));

        let buttons = hardware::read_controller();
        if buttons.ll() {
            smile_x = 0.max(smile_x - 2);
        }
        if buttons.lr() {
            smile_x = 368.min(smile_x + 2);
        }
        if buttons.lu() {
            smile_y = 0.max(smile_y - 2);
        }
        if buttons.ld() {
            smile_y = 208.min(smile_y + 2);
        }

        FRAME.wait_for_new_frame();
    }
}

#![no_main]
#![no_std]

mod assets;

use fixed::types::I10F6;
use vb_graphics::{self as gfx, BgSprite};
use vb_rt::sys::{hardware, vip};

vb_rt::rom_header!("Simple RPG", "SG", "SRPG");
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

    assets::all::load_all(0);

    FRAME.enable_interrupts();

    let mut player = Player {
        x: I10F6::from_num(192),
        y: I10F6::from_num(112),
        dir: Direction::Down,
        moving_frame: None,
    };

    loop {
        player.draw(31);

        let world = vip::WORLDS.index(30);
        world.header().write(vip::WorldHeader::new().with_end(true));

        player.update();

        FRAME.wait_for_new_frame();
    }
}

struct Player {
    x: I10F6,
    y: I10F6,
    dir: Direction,
    moving_frame: Option<usize>,
}

impl Player {
    fn draw(&self, world_index: usize) {
        let sprite = self.sprite();
        let gx = self.x.round().to_num::<i16>() - sprite.width / 2;
        let gy = self.y.round().to_num::<i16>() - sprite.height / 2;
        let world = vip::WORLDS.index(world_index);
        world.header().write(
            vip::WorldHeader::new()
                .with_lon(true)
                .with_ron(true)
                .with_bgm(vip::WorldMode::Normal)
                .with_bg_map_base(sprite.bgmap),
        );
        world.gx().write(gx);
        world.gp().write(0);
        world.gy().write(gy);
        world.mx().write(sprite.x);
        world.mp().write(0);
        world.my().write(sprite.y);
        world.w().write(sprite.width - 1);
        world.h().write(sprite.height - 1);
    }
    const fn sprite(&self) -> BgSprite {
        use assets::all::*;
        if let Some(frame) = self.moving_frame {
            match self.dir {
                Direction::Left => WALK_LEFT,
                Direction::Right => WALK_RIGHT,
                Direction::Up => WALK_UP,
                Direction::Down => WALK_DOWN,
            }
            .frame(frame / 16)
        } else {
            match self.dir {
                Direction::Left => STILL_LEFT,
                Direction::Right => STILL_RIGHT,
                Direction::Up => STILL_UP,
                Direction::Down => STILL_DOWN,
            }
        }
    }
    fn update(&mut self) {
        let buttons = hardware::read_controller();
        let mut xspeed = I10F6::ZERO;
        let mut yspeed = I10F6::ZERO;
        if buttons.ll() {
            xspeed -= I10F6::ONE;
        }
        if buttons.lr() {
            xspeed += I10F6::ONE;
        }
        if buttons.lu() {
            yspeed -= I10F6::ONE;
        }
        if buttons.ld() {
            yspeed += I10F6::ONE;
        }
        if xspeed != 0 && yspeed != 0 {
            xspeed *= I10F6::SQRT_2 / 2;
            yspeed *= I10F6::SQRT_2 / 2;
        }

        if xspeed.is_negative() && yspeed.is_zero() {
            self.dir = Direction::Left;
        }
        if xspeed.is_positive() && yspeed.is_zero() {
            self.dir = Direction::Right;
        }
        if xspeed.is_zero() && yspeed.is_negative() {
            self.dir = Direction::Up;
        }
        if xspeed.is_zero() && yspeed.is_positive() {
            self.dir = Direction::Down;
        }

        if xspeed != 0 || yspeed != 0 {
            self.x = (self.x + xspeed).clamp(I10F6::from_num(8), I10F6::from_num(376));
            self.y = (self.y + yspeed).clamp(I10F6::from_num(8), I10F6::from_num(216));
            self.moving_frame = match self.moving_frame {
                Some(frame) => Some((frame + 1) % 32),
                None => Some(0),
            }
        } else {
            self.moving_frame = None;
        }
    }
}

enum Direction {
    Left,
    Right,
    Up,
    Down,
}

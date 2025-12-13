#![no_std]
#![cfg(target_arch = "v810")]

mod assets;
pub mod text;

use core::sync::atomic::AtomicBool;

pub use assets::{Font, FontCharacter, Image, Mask, Texture};
use vb_rt::sys::{halt, vip};

const PALETTES: [vip::Palette; 4] = [
    vip::Palette::new().with_c1(1).with_c2(2).with_c3(3),
    vip::Palette::new().with_c1(0).with_c2(2).with_c3(3),
    vip::Palette::new().with_c1(1).with_c2(0).with_c3(3),
    vip::Palette::new().with_c1(1).with_c2(2).with_c3(0),
];

pub fn init_display() {
    vip::REST.write(0);

    vip::GPLT0.write(PALETTES[0]);
    vip::JPLT0.write(PALETTES[0]);
    vip::GPLT1.write(PALETTES[1]);
    vip::JPLT1.write(PALETTES[1]);
    vip::GPLT2.write(PALETTES[2]);
    vip::JPLT2.write(PALETTES[2]);
    vip::GPLT3.write(PALETTES[3]);
    vip::JPLT3.write(PALETTES[3]);

    while !vip::DPSTTS.read().scanrdy() {}

    vip::DPCTRL.write(
        vip::DPSTTS
            .read()
            .with_disp(true)
            .with_re(true)
            .with_synce(true),
    );
    vip::XPCTRL.write(vip::XPSTTS.read().with_xpen(true));
}

pub fn set_colors(brta: u8, brtb: u8, brtc: u8) {
    vip::BRTA.write(brta as u16);
    vip::BRTB.write(brtb as u16);
    vip::BRTC.write(brtc as u16);
}

pub fn set_bkcol(value: u16) {
    vip::BKCOL.write(value);
}

pub fn load_character_data(data: &[vip::Character], index: usize) {
    vip::CHARACTERS.write_slice(data, index);
}

pub struct FrameMonitor {
    rendered: AtomicBool,
}

impl FrameMonitor {
    pub const fn new() -> Self {
        Self {
            rendered: AtomicBool::new(false),
        }
    }

    pub fn enable_interrupts(&self) {
        vip::INTENB.write(vip::InterruptFlags::new().with_xpend(true));
        // clear tne NP flag in PSW so interrupts can fire
        unsafe { core::arch::asm!("ldsr r0, psw", options(nomem)) };
    }

    pub fn acknowledge_interrupts(&self) {
        self.rendered
            .store(true, core::sync::atomic::Ordering::Relaxed);
        vip::INTCLR.write(vip::INTPND.read());
    }

    pub fn wait_for_new_frame(&self) {
        while !self.rendered.load(core::sync::atomic::Ordering::Relaxed) {
            halt();
        }
        self.rendered
            .store(false, core::sync::atomic::Ordering::Relaxed);
    }
}

impl Default for FrameMonitor {
    fn default() -> Self {
        Self::new()
    }
}

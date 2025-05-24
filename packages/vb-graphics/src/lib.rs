#![no_std]

use core::sync::atomic::AtomicBool;

use vb_rt::sys::{halt, vip};

pub fn init_display() {
    vip::REST.write(0);

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
        vip::INTENB.write(vip::InterruptFlags::new().with_rfbend(true));
        // clear tne NP flag in PSW so interrupts can fire
        unsafe { core::arch::asm!("ldsr r0, psw") };
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

pub struct Image {
    pub width_cells: u16,
    pub height_cells: u16,
    pub data: &'static [vip::BGCell],
}

impl Image {
    pub fn render_to_bgmap(&self, index: u16, x: u16, y: u16) {
        let map = vip::BG_MAPS.index(index as usize);
        let offsets = (y..y + self.height_cells)
            .flat_map(move |y| (x..x + self.width_cells).map(move |x| y * 64 + x));
        for (src, offset) in self.data.iter().zip(offsets) {
            let dst = map.index(offset as usize);
            dst.write(*src);
        }
    }
}

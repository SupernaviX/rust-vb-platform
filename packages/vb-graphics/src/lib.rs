#![no_std]

use vb_rt::sys::vip::{self, Character};

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

pub fn load_character_data(data: &[Character], index: usize) {
    vip::CHARACTERS.write_slice(data, index);
}

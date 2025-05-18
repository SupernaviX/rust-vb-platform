#![no_main]
#![no_std]

use vb_rt::sys::halt;

vb_rt::rom_header!("Hello World!", "SG", "HIYA");
vb_rt::main!(main);

const DPCTRL: *mut u16 = 0x0005f822 as _;
const XPCTRL: *mut u16 = 0x0005f842 as _;
const BRTA: *mut u16 = 0x0005f824 as _;
const BRTB: *mut u16 = 0x0005f826 as _;
const BRTC: *mut u16 = 0x0005f828 as _;
const BKCOL: *mut u16 = 0x0005f870 as _;

fn main() {
    unsafe {
        DPCTRL.write_volatile(0x0302);
        XPCTRL.write_volatile(0x0002);
        BRTA.write_volatile(32);
        BRTB.write_volatile(64);
        BRTC.write_volatile(32);
        BKCOL.write_volatile(0x0003);
    }
    loop {
        halt();
    }
}

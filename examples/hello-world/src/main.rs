#![no_main]
#![no_std]

use vb_rt::sys::halt;

vb_rt::rom_header!("Hello World!", "SG", "HIYA");
vb_rt::main!({ main() });

fn main() {
    {
        use vb_rt::sys::vip::*;
        DPCTRL.write(
            DisplayFlags::new()
                .with_disp(true)
                .with_re(true)
                .with_synce(true),
        );
        XPCTRL.write(DrawingFlags::new().with_xpen(true));
        BRTA.write(32);
        BRTB.write(64);
        BRTC.write(32);
        BKCOL.write(3);
    }
    loop {
        halt();
    }
}

#![no_main]
#![no_std]

mod assets;

use vb_graphics as gfx;
use vb_sound as snd;

vb_rt::rom_header!("Spooky Music", "SG", "EEEK");
vb_rt::main!({ main() });

static FRAME: gfx::FrameMonitor = gfx::FrameMonitor::new();
static SOUND: snd::SoundPlayer = snd::SoundPlayer::new();
vb_rt::vip_interrupt_handler!({
    FRAME.acknowledge_interrupts();
    SOUND.tick();
});

fn main() {
    gfx::init_display();
    gfx::set_colors(32, 64, 32);
    gfx::set_bkcol(0);
    snd::load_waveforms(&assets::WAVEFORMS);
    SOUND.play(0, assets::TEST_0.as_ptr());
    SOUND.play(1, assets::TEST_1.as_ptr());
    // gfx::load_character_data(&assets::ALL, 0);

    FRAME.enable_interrupts();

    loop {
        FRAME.wait_for_new_frame();
    }
}

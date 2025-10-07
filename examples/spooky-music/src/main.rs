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
    SOUND.play(0, assets::BG_0.as_ptr());
    SOUND.play(1, assets::BG_1.as_ptr());
    SOUND.play(2, assets::BG_2.as_ptr());
    SOUND.play(3, assets::BG_3.as_ptr());
    SOUND.play(5, assets::BG_5.as_ptr());
    // gfx::load_character_data(&assets::ALL, 0);

    FRAME.enable_interrupts();

    loop {
        FRAME.wait_for_new_frame();
    }
}

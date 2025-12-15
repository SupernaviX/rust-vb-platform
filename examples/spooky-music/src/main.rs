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
    snd::WAVEFORMS.load(&assets::CHIRAX_WAVEFORMS);
    snd::CHANNELS[0].play(&assets::CHIRAX_0);
    snd::CHANNELS[1].play(&assets::CHIRAX_1);
    snd::CHANNELS[2].play(&assets::CHIRAX_2);
    snd::CHANNELS[5].play(&assets::CHIRAX_5);
    // gfx::load_character_data(&assets::ALL, 0);

    FRAME.enable_interrupts();

    let mut was_a_pressed = false;
    let mut was_sta_pressed = false;
    loop {
        let pressed = vb_rt::sys::hardware::read_controller();
        let a_pressed = pressed.a();
        if a_pressed && !was_a_pressed {
            if snd::CHANNELS[4].playing_overlay() {
                snd::CHANNELS[2].play_overlay(&assets::HURT_4);
            } else {
                snd::CHANNELS[4].play_overlay(&assets::HURT_4);
            }
        }
        was_a_pressed = a_pressed;
        let sta_pressed = pressed.sta();
        for ch in [
            &snd::CHANNELS[0],
            &snd::CHANNELS[1],
            &snd::CHANNELS[2],
            &snd::CHANNELS[5],
        ] {
            if sta_pressed && !was_sta_pressed {
                ch.pause();
            }
            if !sta_pressed && was_sta_pressed {
                ch.resume();
            }
        }
        was_sta_pressed = sta_pressed;
        FRAME.wait_for_new_frame();
    }
}

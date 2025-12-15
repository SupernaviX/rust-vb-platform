#![no_std]
#![allow(clippy::manual_dangling_ptr)]
#![cfg(target_arch = "v810")]

mod assets;

use core::{
    cell::UnsafeCell,
    ops::{Deref, DerefMut},
    ptr::NonNull,
    sync::atomic::{AtomicBool, AtomicPtr, Ordering::Relaxed},
};

use vb_rt::sys::{VolatilePointer, vsu};

pub static WAVEFORMS: WaveformControl = WaveformControl(AtomicPtr::new(core::ptr::null_mut()));
pub static CHANNELS: [SoundChannel; 6] = [const { SoundChannel::new() }; 6];

pub struct WaveformControl(AtomicPtr<u8>);
impl WaveformControl {
    pub fn load(&self, waveforms: &[u8]) {
        self.0.store(waveforms.as_ptr().cast_mut(), Relaxed);
    }
}

pub struct SoundChannel {
    base: AtomicPtr<u32>,
    overlay: AtomicPtr<u32>,
}
impl SoundChannel {
    const fn new() -> Self {
        Self {
            base: AtomicPtr::new(core::ptr::null_mut()),
            overlay: AtomicPtr::new(core::ptr::null_mut()),
        }
    }

    pub fn play(&self, data: &[u32]) {
        // setting the pointer to the address of the audio to play,
        // plus 1 to signal that we're playing new audio.
        let data = data.as_ptr().map_addr(|a| a | 0x00000001);
        self.base.store(data.cast_mut(), Relaxed);
    }

    pub fn play_overlay(&self, data: &[u32]) {
        // setting the pointer to the address of the audio to play,
        // plus 1 to signal that we're playing new audio,
        let data = data.as_ptr().map_addr(|a| a | 0x00000001);
        self.overlay.store(data.cast_mut(), Relaxed);
    }

    pub fn playing(&self) -> bool {
        !self.base.load(Relaxed).is_null()
    }

    pub fn playing_overlay(&self) -> bool {
        !self.overlay.load(Relaxed).is_null()
    }

    pub fn stop(&self) {
        // setting the pointer to 1 to signal that we are just starting to play nothing
        let data = 0x00000001 as *mut u32;
        self.base.store(data, Relaxed);
    }

    pub fn pause(&self) {
        // setting the pointer to 2 to signal that we're pausing
        let data = 0x00000002 as *mut u32;
        self.base.store(data, Relaxed);
    }

    pub fn resume(&self) {
        // setting the pointer to 3 to signal that we're resumin
        let data = 0x00000003 as *mut u32;
        self.base.store(data, Relaxed);
    }
}

pub struct SoundPlayer(SyncRefCell<[ChannelState; 6]>);
impl SoundPlayer {
    pub const fn new() -> Self {
        Self(SyncRefCell::new([
            ChannelState::new(0),
            ChannelState::new(1),
            ChannelState::new(2),
            ChannelState::new(3),
            ChannelState::new(4),
            ChannelState::new(5),
        ]))
    }

    pub fn tick(&self) {
        self.load_waveforms();
        let mut state = self.0.borrow_mut();
        for channel in state.iter_mut() {
            channel.tick();
        }
    }

    fn load_waveforms(&self) {
        let waveform_set = WAVEFORMS.0.load(Relaxed).cast_const();
        if waveform_set.is_null() {
            return;
        }
        let waveform_bytes = unsafe { (waveform_set as *const u32).read() } as usize;
        let waveform_ptr = unsafe { waveform_set.byte_add(4) };
        let slice = unsafe { core::slice::from_raw_parts(waveform_ptr, waveform_bytes) };
        // Stop all sound, VB requires this to load a new set of waveforms
        vsu::SSTOP.write(0x01);
        vsu::WAVEFORM_BYTES.write_slice(slice, 0);
        WAVEFORMS.0.store(core::ptr::null_mut(), Relaxed);
    }
}

impl Default for SoundPlayer {
    fn default() -> Self {
        Self::new()
    }
}

struct ChannelState {
    channel: usize,
    subs: [SubChannelState; 2],
}
impl ChannelState {
    const fn new(channel: usize) -> Self {
        Self {
            channel,
            subs: [const { SubChannelState::new() }; 2],
        }
    }

    fn tick(&mut self) {
        let controls = &CHANNELS[self.channel];
        let channel = vsu::CHANNELS.index(self.channel);

        self.subs[1].handle_command(controls.overlay.load(Relaxed).cast_const(), channel, false);
        let overlayed = self.subs[1].is_playing();
        self.subs[0].handle_command(controls.base.load(Relaxed).cast_const(), channel, overlayed);

        let base_playing = self.subs[0].tick(channel, overlayed);
        let base_status = if base_playing {
            0x08000000 as *mut u32
        } else {
            core::ptr::null_mut()
        };
        controls.base.store(base_status, Relaxed);

        let overlay_playing = self.subs[1].tick(channel, false);
        let overlay_status = if overlay_playing {
            0x08000000 as *mut u32
        } else {
            core::ptr::null_mut()
        };
        controls.overlay.store(overlay_status, Relaxed);

        if overlayed && !overlay_playing && base_playing {
            // the overlayed sound effect is over, go back to playing the base
            self.subs[0].resume(channel);
        }
    }
}

#[derive(Debug)]
struct SubChannelState {
    playing: *const u32,
    waiting: u32,
    paused: bool,
    shadowed: [u8; 8],
}

impl SubChannelState {
    const fn new() -> Self {
        Self {
            playing: core::ptr::null(),
            waiting: 0,
            paused: false,
            shadowed: [0; 8],
        }
    }

    fn handle_command(
        &mut self,
        cmd: *const u32,
        channel: VolatilePointer<vsu::Channel>,
        silent: bool,
    ) {
        match cmd.addr() & 0x03 {
            1 => {
                // start playing
                self.playing = cmd.map_addr(|a| a & 0x07fffffc);
                self.waiting = 0;
                self.paused = false;
                if !silent {
                    // SILENCE!!!
                    channel.interval().write(vsu::IntervalData::new());
                }
            }
            2 => {
                // pause
                self.paused = true;
                if !silent {
                    channel.env_lo().write(vsu::EnvelopeLowData::new());
                }
            }
            3 => {
                // resume
                self.paused = false;
                if !silent {
                    channel.env_lo().cast::<u8>().write(self.shadowed[4]);
                }
            }
            _ => {} // do nothing
        }
    }

    fn is_playing(&self) -> bool {
        !self.playing.is_null() && !self.paused
    }

    fn resume(&self, channel: VolatilePointer<vsu::Channel>) {
        for (offset, value) in self.shadowed.into_iter().enumerate() {
            let field = unsafe { channel.field::<u8>(offset << 2) };
            field.write(value);
        }
    }

    fn tick(&mut self, channel: VolatilePointer<vsu::Channel>, silent: bool) -> bool {
        if self.paused || self.playing.is_null() {
            // Not playing audio right now.
            return false;
        } else if self.waiting > 0 {
            // We're waiting at least one frame before playing more.
            self.waiting -= 1;
            return true;
        }
        loop {
            let event = ChannelEvent::decode(unsafe { self.playing.read() });
            match event {
                ChannelEvent::Done => {
                    if !silent {
                        channel
                            .interval()
                            .write(vsu::IntervalData::new().with_enabled(false));
                    }
                    self.playing = core::ptr::null_mut();
                    return false;
                }
                ChannelEvent::Wait { frames } => {
                    self.playing = unsafe { self.playing.offset(1) };
                    self.waiting = frames;
                    return true;
                }
                ChannelEvent::Write { offset, value } => {
                    self.playing = unsafe { self.playing.offset(1) };
                    if !silent {
                        let field = unsafe { channel.field::<u8>(offset as usize) };
                        field.write(value);
                    }
                    // track old values in case we pause or get overridden
                    self.shadowed[offset as usize >> 2] = value;
                }
                ChannelEvent::Jump { offset } => {
                    self.playing = unsafe { self.playing.offset(offset) };
                }
            }
        }
    }
}

#[derive(Debug)]
enum ChannelEvent {
    Done,
    Wait { frames: u32 },
    Write { offset: u8, value: u8 },
    Jump { offset: isize },
}
impl ChannelEvent {
    fn decode(value: u32) -> Self {
        let [b0, b1, b2, b3] = value.to_le_bytes();
        match b0 {
            0 => {
                let frames = u32::from_le_bytes([b1, b2, b3, 0]);
                if frames > 0 {
                    Self::Wait { frames: frames - 1 }
                } else {
                    Self::Done
                }
            }
            1 => Self::Write {
                offset: b2,
                value: b3,
            },
            2 => {
                let high_byte = if b3 >= 128 { 255 } else { 0 };
                let offset = i32::from_le_bytes([b1, b2, b3, high_byte]);
                Self::Jump {
                    offset: offset as isize,
                }
            }
            _ => Self::Done,
        }
    }
}

struct SyncRefCell<T> {
    value: UnsafeCell<T>,
    taken: AtomicBool,
}
impl<T> SyncRefCell<T> {
    const fn new(value: T) -> Self {
        Self {
            value: UnsafeCell::new(value),
            taken: AtomicBool::new(false),
        }
    }
    fn borrow_mut(&self) -> RefMut<'_, T> {
        if self.taken.load(Relaxed) {
            panic!("borrowed twice");
        }
        self.taken.store(true, Relaxed);
        // SAFETY: UnsafeCell's content can't be null
        let value = unsafe { NonNull::new_unchecked(self.value.get()) };
        RefMut {
            value,
            taken: &self.taken,
        }
    }
}
unsafe impl<T> Sync for SyncRefCell<T> {}

struct RefMut<'a, T> {
    value: NonNull<T>,
    taken: &'a AtomicBool,
}

impl<T> Deref for RefMut<'_, T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        unsafe { self.value.as_ref() }
    }
}
impl<T> DerefMut for RefMut<'_, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { self.value.as_mut() }
    }
}
impl<T> Drop for RefMut<'_, T> {
    fn drop(&mut self) {
        self.taken.store(false, Relaxed);
    }
}

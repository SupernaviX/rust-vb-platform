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

pub use assets::WaveformData;
use vb_rt::sys::{VolatilePointer, vsu};

pub static WAVEFORMS: WaveformControl = WaveformControl(AtomicPtr::new(core::ptr::null_mut()));
pub static CHANNELS: [SoundChannel; 6] = [const { SoundChannel::new() }; 6];

pub struct WaveformControl(AtomicPtr<u8>);
impl WaveformControl {
    pub fn load(&self, waveforms: &[u8]) {
        self.0.store(waveforms.as_ptr().cast_mut(), Relaxed);
    }
}

enum Command {
    Play(*mut u32, Priority),
    Pause,
    Resume,
}

struct Controller(AtomicPtr<u32>);
impl Controller {
    pub const fn new() -> Self {
        Self(AtomicPtr::new(core::ptr::null_mut()))
    }

    pub fn play(&self, data: &[u32], priority: Priority) {
        let flags = ((priority as u8) as usize) << 28 | 0x00000001;
        let value = data.as_ptr().map_addr(|a| a | flags).cast_mut();
        self.store(value);
    }

    pub fn stop(&self) {
        self.store(0x00000001 as *mut u32);
    }

    pub fn pause(&self) {
        self.store(0x00000002 as *mut u32);
    }

    pub fn resume(&self) {
        self.store(0x00000003 as *mut u32);
    }

    pub fn set_status(&self, priority: Option<Priority>) {
        let status = match priority {
            Some(p) => (((p as u8) as u32) << 28 | 0x08000000) as *mut u32,
            None => core::ptr::null_mut(),
        };
        self.store(status);
    }

    pub fn playing(&self) -> bool {
        !self.load().is_null()
    }

    pub fn priority(&self) -> Option<Priority> {
        let raw = self.load();
        if raw.is_null() {
            None
        } else {
            Some(Priority::from((raw.addr() >> 28) as u8))
        }
    }

    pub fn command(&self) -> Option<Command> {
        let value = self.load();
        match value.addr() & 0x03 {
            0x01 => {
                let pointer = value.map_addr(|v| v & 0x07fffffc);
                let priority = Priority::from((value.addr() >> 28) as u8);
                Some(Command::Play(pointer, priority))
            }
            0x02 => Some(Command::Pause),
            0x03 => Some(Command::Resume),
            _ => None,
        }
    }

    fn load(&self) -> *mut u32 {
        self.0.load(Relaxed)
    }

    fn store(&self, value: *mut u32) {
        self.0.store(value, Relaxed);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Priority {
    Low,
    Normal,
    High,
}
impl From<u8> for Priority {
    fn from(value: u8) -> Self {
        match value {
            2 => Self::High,
            1 => Self::Normal,
            _ => Self::Low,
        }
    }
}
impl From<Priority> for u8 {
    fn from(value: Priority) -> Self {
        match value {
            Priority::Low => 0,
            Priority::Normal => 1,
            Priority::High => 2,
        }
    }
}

pub struct SoundChannel {
    base: Controller,
    overlay: Controller,
}
impl SoundChannel {
    const fn new() -> Self {
        Self {
            base: Controller::new(),
            overlay: Controller::new(),
        }
    }

    pub fn play(&self, data: &[u32]) {
        self.base.play(data, Priority::Normal);
    }

    pub fn play_overlay(&self, data: &[u32]) -> bool {
        self.play_overlay_priority(data, Priority::Normal)
    }

    pub fn play_overlay_priority(&self, data: &[u32], priority: Priority) -> bool {
        if self.overlay.priority().is_none_or(|p| p < priority) {
            self.overlay.play(data, priority);
            true
        } else {
            false
        }
    }

    pub fn playing(&self) -> bool {
        self.base.playing()
    }

    pub fn playing_overlay(&self) -> bool {
        self.overlay.playing()
    }

    pub fn stop(&self) {
        self.base.stop();
    }

    pub fn pause(&self) {
        self.base.pause();
    }

    pub fn resume(&self) {
        self.base.resume();
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

        if let Some(cmd) = controls.overlay.command() {
            self.subs[1].handle_command(cmd, channel, false);
        }
        let overlayed = self.subs[1].is_playing();
        if let Some(cmd) = controls.base.command() {
            self.subs[0].handle_command(cmd, channel, overlayed);
        }

        let base_playing = self.subs[0].tick(channel, overlayed);
        controls.base.set_status(base_playing);

        let overlay_playing = self.subs[1].tick(channel, false);
        controls.overlay.set_status(overlay_playing);

        if overlayed && overlay_playing.is_none() && base_playing.is_some() {
            // the overlayed sound effect is over, go back to playing the base
            self.subs[0].resume(channel);
        }
    }
}

#[derive(Debug)]
struct SubChannelState {
    playing: *const u32,
    priority: Priority,
    waiting: u32,
    paused: bool,
    shadowed: [u8; 8],
}

impl SubChannelState {
    const fn new() -> Self {
        Self {
            playing: core::ptr::null(),
            priority: Priority::Low,
            waiting: 0,
            paused: false,
            shadowed: [0; 8],
        }
    }

    fn handle_command(
        &mut self,
        cmd: Command,
        channel: VolatilePointer<vsu::Channel>,
        silent: bool,
    ) {
        match cmd {
            Command::Play(playing, priority) => {
                // start playing
                self.playing = playing;
                self.priority = priority;
                self.waiting = 0;
                self.paused = false;
                if !silent {
                    // SILENCE!!!
                    channel.interval().write(vsu::IntervalData::new());
                }
            }
            Command::Pause => {
                // pause
                self.paused = true;
                if !silent {
                    channel.env_lo().write(vsu::EnvelopeLowData::new());
                }
            }
            Command::Resume => {
                // resume
                self.paused = false;
                if !silent {
                    channel.env_lo().cast::<u8>().write(self.shadowed[4]);
                }
            }
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

    fn tick(&mut self, channel: VolatilePointer<vsu::Channel>, silent: bool) -> Option<Priority> {
        if self.paused || self.playing.is_null() {
            // Not playing audio right now.
            return None;
        } else if self.waiting > 0 {
            // We're waiting at least one frame before playing more.
            self.waiting -= 1;
            return Some(self.priority);
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
                    return None;
                }
                ChannelEvent::Wait { frames } => {
                    self.playing = unsafe { self.playing.offset(1) };
                    self.waiting = frames;
                    return Some(self.priority);
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

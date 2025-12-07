#![no_std]

mod assets;

use core::{
    cell::UnsafeCell,
    ops::{Deref, DerefMut},
    ptr::NonNull,
    sync::atomic::{AtomicBool, AtomicPtr, Ordering::Relaxed},
};

use vb_rt::sys::vsu;

pub fn load_waveforms(waveforms: &[[u8; 32]]) {
    for (index, waveform) in waveforms.iter().enumerate() {
        vsu::WAVEFORMS.index(index).write_slice(waveform, 0);
    }
}

pub static CHANNELS: [SoundChannel; 6] =
    [const { SoundChannel(AtomicPtr::new(core::ptr::null_mut())) }; 6];

pub struct SoundChannel(AtomicPtr<u32>);
impl SoundChannel {
    pub fn play(&self, data: &[u32]) {
        // setting the pointer to the address of the audio to play,
        // plus 1 to signal that we're playing new audio.
        let data = unsafe { data.as_ptr().byte_offset(1) };
        self.0.store(data.cast_mut(), Relaxed);
    }

    pub fn stop(&self) {
        self.0.store(core::ptr::null_mut(), Relaxed);
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
        let mut state = self.0.borrow_mut();
        for channel in state.iter_mut() {
            channel.tick();
        }
    }
}

impl Default for SoundPlayer {
    fn default() -> Self {
        Self::new()
    }
}

struct ChannelState {
    channel: usize,
    waiting: u32,
}
impl ChannelState {
    const fn new(channel: usize) -> Self {
        Self {
            channel,
            waiting: 0,
        }
    }

    pub fn tick(&mut self) {
        let control = &CHANNELS[self.channel].0;
        let mut ptr = control.load(Relaxed).cast_const();
        if ptr.is_null() {
            // Not playing audio right now.
            return;
        } else if ptr.addr() & 1 != 0 {
            // By setting the low bit, caller requested to play new audio.
            // Clear that bit and ignore any waits from before.
            ptr = unsafe { ptr.byte_offset(-1) }
        } else if self.waiting > 0 {
            // We're waiting at least one frame before playing more.
            self.waiting -= 1;
            return;
        }
        let channel = vsu::CHANNELS.index(self.channel);
        loop {
            let event = ChannelEvent::decode(unsafe { ptr.read() });
            match event {
                ChannelEvent::Done => {
                    channel
                        .interval()
                        .write(vsu::IntervalData::new().with_enabled(false));
                    control.store(core::ptr::null_mut(), Relaxed);
                    return;
                }
                ChannelEvent::Wait { frames } => {
                    ptr = unsafe { ptr.offset(1) };
                    control.store(ptr.cast_mut(), Relaxed);
                    self.waiting = frames;
                    return;
                }
                ChannelEvent::Write { offset, value } => {
                    ptr = unsafe { ptr.offset(1) };
                    let field = unsafe { channel.field::<u8>(offset as usize) };
                    field.write(value);
                }
                ChannelEvent::Jump { offset } => {
                    ptr = unsafe { ptr.offset(offset) };
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

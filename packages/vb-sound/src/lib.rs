#![no_std]

use core::sync::atomic::{AtomicPtr, AtomicU32, Ordering::Relaxed};

use vb_rt::sys::{VolatilePointer, vsu};

pub struct SoundPlayer {
    channels: [ChannelState; 6],
}

impl SoundPlayer {
    pub const fn new() -> Self {
        Self {
            channels: [
                ChannelState::new(0),
                ChannelState::new(1),
                ChannelState::new(2),
                ChannelState::new(3),
                ChannelState::new(4),
                ChannelState::new(5),
            ],
        }
    }

    pub fn init(&self) {
        #[rustfmt::skip]
        let square = [
             0,  0,  0,  0,  0,  0,  0,  0,
             0,  0,  0,  0,  0,  0,  0,  0,
            62, 62, 62, 62, 63, 63, 63, 63,
            62, 62, 62, 62, 63, 63, 63, 63,
        ];
        vsu::WAVEFORMS.index(0).write_slice(&square, 0);
    }

    pub fn play(&self, channel: usize, program: *const u32) {
        assert!(!program.is_null());
        let signalling_program = unsafe { program.cast::<u8>().add(1).cast::<u32>() };
        self.channels[channel]
            .program
            .store(signalling_program.cast_mut(), Relaxed);
    }

    pub fn stop(self, channel: usize) {
        self.channels[channel]
            .program
            .store(core::ptr::null_mut(), Relaxed);
    }

    pub fn tick(&self) {
        for channel in &self.channels {
            channel.tick()
        }
    }
}

impl Default for SoundPlayer {
    fn default() -> Self {
        Self::new()
    }
}

struct ChannelState {
    channel: VolatilePointer<vsu::Channel>,
    program: AtomicPtr<u32>,
    waiting: AtomicU32,
}
impl ChannelState {
    const fn new(channel: usize) -> Self {
        Self {
            channel: vsu::CHANNELS.index(channel),
            program: AtomicPtr::new(core::ptr::null_mut()),
            waiting: AtomicU32::new(0),
        }
    }

    fn tick(&self) {
        let mut program = self.program.load(Relaxed).cast_const();
        if program.is_null() {
            return;
        } else if program.addr() & 1 != 0 {
            program = unsafe { program.byte_offset(-1) };
        } else {
            let waiting = self.waiting.load(Relaxed);
            if waiting > 0 {
                self.waiting.store(waiting - 1, Relaxed);
                return;
            }
        }
        loop {
            let event = ChannelEvent::decode(unsafe { program.read() });
            match event {
                ChannelEvent::Done => {
                    self.channel
                        .interval()
                        .write(vsu::IntervalData::new().with_enabled(false));
                    self.program.store(core::ptr::null_mut(), Relaxed);
                    return;
                }
                ChannelEvent::Wait { frames } => {
                    program = unsafe { program.offset(1) };
                    self.program.store(program.cast_mut(), Relaxed);
                    self.waiting.store(frames, Relaxed);
                    return;
                }
                ChannelEvent::Write { offset, value } => {
                    program = unsafe { program.offset(1) };
                    let field = unsafe { self.channel.field::<u8>(offset as usize) };
                    field.write(value);
                }
                ChannelEvent::Jump { offset } => {
                    program = unsafe { program.offset(offset) };
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
                let offset = i32::from_le_bytes([b1, b2, b3, 0]);
                Self::Jump {
                    offset: offset as isize,
                }
            }
            _ => Self::Done,
        }
    }
}

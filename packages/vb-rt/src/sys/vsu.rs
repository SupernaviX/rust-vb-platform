use bitfield_struct::bitfield;

use crate::sys::{
    VolatilePointer,
    volatile::{mmio, mmstruct},
};

/** A waveform, made out of 32 6-bit unsigned integers. */
pub type Waveform = [u8; 32];

/** Modulation data used by channel 5, made out of 32 8-bit signed integers */
pub type Modulation = [i8; 32];

mmio! {
    /** A set of waveforms for PCM channels. */
    pub const WAVEFORMS: [Waveform; 5], align 4 = 0x01000000;
    /** Modulation data used by channel 5. */
    pub const MODULATION: Modulation, align 4 = 0x01000280;
}

#[bitfield(u8)]
pub struct IntervalData {
    /// The time to wait before deactivating the channel.
    #[bits(5)]
    pub interval: u8,
    /// Set to schedule automatic channel deactivation.
    pub auto: bool,
    _pad: bool,
    /// Set to enable sound generation.
    pub enabled: bool,
}

#[bitfield(u8)]
pub struct LevelData {
    /// The right volume.
    #[bits(4)]
    pub right: u8,
    /// The left volume.
    #[bits(4)]
    pub left: u8,
}

macro_rules! bool_enum {
    (
        $(#[$enum_attr:meta])*
        $enum_vis:vis $name:ident($(#[$false_attr:meta])* $false:ident, $(#[$true_attr:meta])* $true:ident)
    ) => {
            $(#[$enum_attr])*
            #[derive(Debug, Clone, Copy, PartialEq, Eq)]
            #[repr(u8)]
            pub enum $name {
                $(#[$false_attr])*
                $false = 0,
                $(#[$true_attr])*
                $true = 1,
            }
            impl $name {
                const fn into_bits(self) -> u8 {
                    self as _
                }

                const fn from_bits(value: u8) -> Self {
                    match value {
                        0 => Self::$false,
                        _ => Self::$true,
                    }
                }
            }
    };
}

bool_enum! {
    pub Direction(Shrink, Grow)
}

#[bitfield(u8)]
pub struct EnvelopeLowData {
    /// Specifies the time between envelope modifications.
    #[bits(3)]
    pub interval: u8,
    /// Specifies the direction in which the envelope is modified.
    #[bits(1)]
    pub dir: Direction,
    /// The initial and reload value of the envelope.
    #[bits(4)]
    pub value: u8,
}

bool_enum! {
    /// Controls the type of frequency modification.
    pub ModFunc(Sweep, Modulation)
}

#[bitfield(u8)]
pub struct EnvelopeHighData {
    /// When set, envelope modification is enabled.
    pub enable: bool,
    /// Specifies whether envelope modification loops.
    pub repeat: bool,
    #[bits(2)]
    _pad0: u8,
    /// Specifies the frequency modification function.
    #[bits(1)]
    pub mod_func: ModFunc,
    /// Specifies whether modulation loops.
    pub mod_repeat: bool,
    /// When set, frequency modification is enabled.
    pub mod_enable: bool,
    _pad1: bool,
}

#[bitfield(u8)]
pub struct NoiseData {
    /// When set, envelope modification is enabled.
    pub enable: bool,
    /// Specifies whether envelope modification loops.
    pub repeat: bool,
    #[bits(2)]
    _pad0: u8,
    /// Specifies the bit to use in noise generation.
    #[bits(3)]
    pub tap: u8,
    _pad1: bool,
}

bool_enum! {
    pub Clock (
        /// ~0.96ms (1041.6 Hz)
        Clock0,
        /// ~7.68ms (130.2 Hz)
        Clock1
    )
}

#[bitfield(u8)]
pub struct SweepModData {
    /// Specifies the sweep shift amount.
    #[bits(3)]
    pub shift: u8,
    /// Specifies the sweep direction.
    #[bits(1)]
    pub dir: Direction,
    /// Specifies the modification interval.
    #[bits(3)]
    pub interval: u8,
    /// Specifies the base clock for the frequency modification interval.
    #[bits(1)]
    pub clock: Clock,
}

mmstruct! {
    #[repr(C, align(4))]
    #[derive(Copy, Clone)]
    pub struct Channel overalign_fields(4){
        /// Controls the channel's interval.
        pub interval: IntervalData,
        /// Controls the channel's stereo levels (volume).
        pub level: LevelData,
        /// The lower 8 bits of the channel's frequency.
        pub freq_lo: u8,
        /// The upper 3 bits of the channel's frequency.
        pub freq_hi: u8,
        /// Together with env_hi, controls the channel's envelope behavior.
        pub env_lo: EnvelopeLowData,
        /// Together with env_lo, controls the channel's envelope behavior.
        pub env_hi: EnvelopeHighData,
        /// Controls which waveform the channel will play.
        pub wave: u8,
        /// Controls the frequency sweep and modulation features of channel 5.
        pub swp_mod: SweepModData,
        _padding: [u8; 8],
    }
}
const _: () = assert!(size_of::<Channel>() == 0x40);

impl VolatilePointer<Channel> {
    /// Helper to write the channel's frequency in one call.
    pub fn freq_write(self, val: u16) {
        self.freq_lo().write(val as u8);
        self.freq_hi().write((val >> 8) as u8);
    }

    /// Controls pseudorandom noise generation on channel 6.
    pub fn noise(self) -> VolatilePointer<NoiseData> {
        // SAFETY: the same offset is used for envelope high data in PCM channels
        // and the noise control in the noise channel. Easier to just cast in a helper
        // than to define two separate Channel structs.
        unsafe { core::mem::transmute(self.env_hi()) }
    }
}

mmio! {
    pub const CHANNELS: [Channel; 6] = 0x01000400;
    pub const SSTOP: u8 = 0x01000580;
}

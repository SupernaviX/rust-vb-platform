use bitfield_struct::bitfield;

use super::volatile::mmio;

/// The lower 8 bits of serial (controller) data
#[bitfield(u8)]
pub struct SerialDataLow {
    /// Low battery
    pub pwr: bool,
    /// Signature (always set)
    pub sgn: bool,
    /// A button
    pub a: bool,
    /// B button
    pub b: bool,
    /// Right trigger
    pub rt: bool,
    /// Left trigger
    pub lt: bool,
    /// Right D-pad up
    pub ru: bool,
    /// Right D-pad right
    pub rr: bool,
}

/// The higher 8 bits of serial (controller) data
#[bitfield(u8)]
pub struct SerialDataHigh {
    /// Left D-pad right
    pub lr: bool,
    /// Left D-pad left
    pub ll: bool,
    /// Left D-pad down
    pub ld: bool,
    /// Left D-pad up
    pub lu: bool,
    /// Start button
    pub sta: bool,
    /// Select button
    pub sel: bool,
    /// Right D-pad left
    pub rl: bool,
    /// Right D-pad down
    pub rd: bool,
}

mmio! {
    pub const SDLR: SerialDataLow = 0x02000010;
    pub const SDHR: SerialDataHigh = 0x02000014;
}

#[bitfield(u8)]
pub struct SerialControlData {
    /// When set, aborts hardware reads
    pub s_abt_dis: bool,
    /// Set while a hardware read is in progress
    pub si_stat: bool,
    /// Set to initiate a hardware read
    pub hw_si: bool,
    _padding0: bool,
    /// Sends the inverse read bit to the game pad
    pub soft_ck: bool,
    /// When set, reset a software read
    pub para_si: bool,
    _padding1: bool,
    /// Enables the key input interrupt
    pub k_int_inh: bool,
}
mmio! {
    pub const SCR: SerialControlData = 0x02000028;
}

// Real hardware does not support stdout, it is a feature of the Lemur emulator.
mmio! {
    pub const STDOUT: u8 = 0x02000030;
}

// Real hardware ignores writes to this address.
// The Lemur emulator lets games write the address of a null-terminated string to this address,
// to emit a custom marker in the profiler.
mmio! {
    pub const MARKER: *const core::ffi::c_char = 0x02000038;
}

// utility for reading controller data
pub fn read_controller() -> GamePadData {
    SCR.write(
        SerialControlData::new()
            .with_k_int_inh(true)
            .with_hw_si(true),
    );
    while SCR.read().si_stat() {}
    let lo: u8 = SDLR.read().into();
    let hi: u8 = SDHR.read().into();
    GamePadData::from((lo as u16) | ((hi as u16) << 8))
}

#[bitfield(u16)]
pub struct GamePadData {
    /// Low battery
    pub pwr: bool,
    /// Signature (always set)
    pub sgn: bool,
    /// A button
    pub a: bool,
    /// B button
    pub b: bool,
    /// Right trigger
    pub rt: bool,
    /// Left trigger
    pub lt: bool,
    /// Right D-pad up
    pub ru: bool,
    /// Right D-pad right
    pub rr: bool,
    /// Left D-pad right
    pub lr: bool,
    /// Left D-pad left
    pub ll: bool,
    /// Left D-pad down
    pub ld: bool,
    /// Left D-pad up
    pub lu: bool,
    /// Start button
    pub sta: bool,
    /// Select button
    pub sel: bool,
    /// Right D-pad left
    pub rl: bool,
    /// Right D-pad down
    pub rd: bool,
}

pub fn emit_profiling_marker(name: &core::ffi::CStr) {
    MARKER.write(name.as_ptr());
}

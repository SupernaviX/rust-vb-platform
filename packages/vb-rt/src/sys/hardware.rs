use bitfield_struct::bitfield;

use crate::sys::util::bool_enum;

use super::volatile::mmio;

/// The communication (link cable) control register
#[bitfield(u8)]
pub struct CommunicationControlRegister {
    _padding0: bool,
    /// Set while a communication is underway
    pub c_stat: bool,
    /// When set, a communication operation begins
    pub c_start: bool,
    _padding1: bool,
    /// Which communication clock signal to sync on
    #[bits(1)]
    pub c_clk_sel: CommunicationClock,
    #[bits(2)]
    _padding2: u8,
    /// Set to ack/inhibit the communication interrupt for communications (clear to enable it)
    pub c_int_inh: bool,
}

bool_enum! {
    /// A VB unit's communication clock signal
    pub CommunicationClock(
        /// Our own clock signal
        Internal,
        /// Another unit's clock signal
        External
    )
}

/// The COMCNT control register
#[bitfield(u8)]
pub struct CommunicationControlSignalRegister {
    /// The current manual signal status
    pub cc_rd: bool,
    /// Set the manual signal status
    pub cc_wr: bool,
    /// Sampled from the signal after a communication operation completes
    pub cc_smp: bool,
    /// The automated signal status to use after a communication operation completes
    pub cc_sig: bool,
    /// This must equal cc_sig to raise the communication interrupt for signals.
    pub cc_int_lev: bool,
    #[bits(2)]
    padding: u8,
    /// Set to ack/inhibit the communication interrupt for signals (clear to enable it)
    pub cc_int_inh: bool,
}

mmio! {
    /// The communication (link cable) control register
    pub const CCR: CommunicationControlRegister = 0x02000000;

    /// The COMCNT control register
    pub const CCSR: CommunicationControlSignalRegister = 0x02000004;

    /// Transmitted data register
    pub const CDTR: u8 = 0x02000008;

    /// Received data register
    pub const CDRR: u8 = 0x0200000C;
}

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
    /// Set to ack/inhibit the key input interrupt (clear to enable it)
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

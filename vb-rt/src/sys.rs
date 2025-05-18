use core::arch::asm;

/// Enable interrupts
#[inline(always)]
pub fn cli() {
    unsafe { asm!("cli") };
}

/// Disable interrupts
#[inline(always)]
pub fn sei() {
    unsafe { asm!("sei") };
}

/// Halt the CPU until the next interrupt.
#[inline(always)]
pub fn halt() {
    unsafe { asm!("halt") };
}

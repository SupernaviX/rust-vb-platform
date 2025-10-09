// Duplicate any builtins from the clang runtime here.
// Unfortunately, rust doesn't have an easy way to link against that directly.

use core::arch::global_asm;

global_asm!(include_str!("builtins/__memcpy_wordaligned.S"));

unsafe extern "C" {
    #[link_name = "__memcpy_wordaligned"]
    pub unsafe fn memcpy_wordaligned(dest: *mut u8, src: *const u8, count: usize) -> *mut u8;
}

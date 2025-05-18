// Duplicate any builtins from the clang runtime here.
// Unfortunately, rust doesn't have an easy way to link against that directly.

use core::arch::global_asm;

global_asm!(include_str!("builtins/__memcpy_wordaligned.S"));

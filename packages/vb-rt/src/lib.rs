#![no_std]

mod builtins;
pub mod macros;
mod reset;
pub mod sys;

use core::panic::PanicInfo;

#[panic_handler]
fn panic(_panic: &PanicInfo<'_>) -> ! {
    // TODO: display the error message
    loop {
        sys::halt();
    }
}

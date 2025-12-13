#![no_std]
#![feature(macro_metavar_expr_concat)]
#![cfg(target_arch = "v810")]

mod builtins;
pub mod macros;
mod reset;
pub mod stdio;
pub mod sys;

use core::panic::PanicInfo;

#[panic_handler]
fn panic(panic: &PanicInfo<'_>) -> ! {
    println!("{panic}");
    loop {
        sys::halt();
    }
}

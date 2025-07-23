macro_rules! handler {
    ($handler:ident) => {
        unsafe extern "Rust" {
            unsafe fn $handler();
        }
        core::arch::global_asm!("
            .section .handlers.{handler},\"ax\",@progbits
            .globl __handle_{handler}
            .type __handle_{handler},@function
        __handle_{handler}:
            movhi hi({handler}), r0, r1
            movea lo({handler}), r1, r1
            jmp [r1]
            /* for padding */
            nop
            nop
            nop
        .L__handle_{handler}_end:
            .size __handle_{handler}, .L__handle_{handler}_end-__handle_{handler}
        ", handler = sym $handler);

    }
}

macro_rules! in2rupt_handlers{
    () => {};
    ($(#[$attr:meta])* $macro:ident($handler:ident); $($rest:tt)*) => {
        handler!($handler);

        #[macro_export]
        $(#[$attr])*
        macro_rules! $macro {
            ($body:block) => {
                #[interrupt]
                #[unsafe(no_mangle)]
                pub fn $handler() {
                    $body
                }
            };
        }
        in2rupt_handlers!($($rest)*);
    };
}

#[interrupt]
#[unsafe(no_mangle)]
pub fn default_handler() {}

in2rupt_handlers! {
    /// Define a handler to run on game pad interrupts.
    /// Note that these interrupts never fire on stock hardware.
    game_pad_interrupt_handler(_vb_rt_game_pad_handler);

    /// Define a handler to run on timer interrupts.
    timer_interrupt_handler(_vb_rt_timer_handler);

    /// Define a handler to run on game pak interrupts.
    /// Note that these interrupts never fire on stock hardware.
    game_pak_interrupt_handler(_vb_rt_game_pak_handler);

    /// Define a handler to run on communication interrupts.
    communication_interrupt_handler(_vb_rt_communication_handler);

    /// Define a handler to run on VIP interrupts.
    vip_interrupt_handler(_vb_rt_vip_handler);

    /// Define a handler to run on floating point exceptions.
    fp_exception_handler(_vb_rt_fp_exception_handler);

    /// Define a handler to run on divide by zero exceptions.
    divide_by_zero_exception_handler(_vb_rt_divide_by_zero_handler);

    /// Define a handler to run on illegal opcode exceptions.
    illegal_opcode_exception_handler(_vb_rt_illegal_opcode_handler);

    /// Define a handler to run when the TRAP instruction is fired with an immediate less than 16.
    lo_trap_handler(_vb_rt_lo_trap_handler);

    /// Define a handler to run when the TRAP instruction is fired with an immediate 16 or greater.
    hi_trap_handler(_vb_rt_hi_trap_handler);

    /// Define a handler to run when a hardware breakpoint is hit.
    address_trap_handler(_vb_rt_address_trap_handler);

    /// Define a handler to run when an exception occurs while processing an interrupt.
    duplexed_exception_handler(_vb_rt_duplexed_exception_handler);
}
handler!(_vb_rt_reset);

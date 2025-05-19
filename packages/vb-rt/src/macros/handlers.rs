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

macro_rules! interrupt_handler {
    ($handler:ident, $macro:ident) => {
        handler!($handler);

        #[macro_export]
        macro_rules! $macro {
            ($body:block) => {
                #[interrupt]
                #[unsafe(no_mangle)]
                pub fn $handler() {
                    $body
                }
            }
        }
    }
}

#[interrupt]
#[unsafe(no_mangle)]
pub fn default_handler() {}

interrupt_handler!(_vb_rt_game_pad_handler, game_pad_interrupt_handler);
interrupt_handler!(_vb_rt_timer_handler, timer_interrupt_handler);
interrupt_handler!(_vb_rt_game_pak_handler, game_pak_interrupt_handler);
interrupt_handler!(_vb_rt_communication_handler, communication_interrupt_handler);
interrupt_handler!(_vb_rt_vip_handler, vip_interrupt_handler);
interrupt_handler!(_vb_rt_fp_exception_handler, fp_exception_handler);
interrupt_handler!(_vb_rt_divide_by_zero_handler, divide_by_zero_exception_handler);
interrupt_handler!(_vb_rt_illegal_opcode_handler, illegal_opcode_exception_handler);
interrupt_handler!(_vb_rt_lo_trap_handler, lo_trap_handler);
interrupt_handler!(_vb_rt_hi_trap_handler, hi_trap_handler);
interrupt_handler!(_vb_rt_address_trap_handler, address_trap_handler);
interrupt_handler!(_vb_rt_duplexed_exception_handler, duplexed_exception_handler);
handler!(_vb_rt_reset);

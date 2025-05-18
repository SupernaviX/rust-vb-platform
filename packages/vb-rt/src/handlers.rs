#[macro_export]
macro_rules! register_handler {
    ($handler:ident) => {
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

#[interrupt]
#[unsafe(no_mangle)]
pub fn default_handler() {}

unsafe extern "Rust" {
    unsafe fn vb_game_pad_handler();
    unsafe fn vb_timer_handler();
    unsafe fn vb_game_pak_handler();
    unsafe fn vb_communication_handler();
    unsafe fn vb_vip_handler();
    unsafe fn vb_fp_exception_handler();
    unsafe fn vb_divide_by_zero_handler();
    unsafe fn vb_illegal_opcode_handler();
    unsafe fn vb_lo_trap_handler();
    unsafe fn vb_hi_trap_handler();
    unsafe fn vb_address_trap_handler();
    unsafe fn vb_duplexed_exception_handler();
}

register_handler!(vb_game_pad_handler);
register_handler!(vb_timer_handler);
register_handler!(vb_game_pak_handler);
register_handler!(vb_communication_handler);
register_handler!(vb_vip_handler);
register_handler!(vb_fp_exception_handler);
register_handler!(vb_divide_by_zero_handler);
register_handler!(vb_illegal_opcode_handler);
register_handler!(vb_lo_trap_handler);
register_handler!(vb_hi_trap_handler);
register_handler!(vb_address_trap_handler);
register_handler!(vb_duplexed_exception_handler);

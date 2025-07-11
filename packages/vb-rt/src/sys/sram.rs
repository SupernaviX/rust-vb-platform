use crate::sys::volatile::mmio;

mmio! {
    /**
    Only even-numbered addresses in SRAM are usable.
    This pointer wrapper lets you access the even addresses of SRAM
    as if it were a contiguous memory region.
    */
    pub const SRAM: [u8; 8192], align 2 = 0x06000000;
}

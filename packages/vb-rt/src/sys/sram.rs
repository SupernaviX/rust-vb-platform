use crate::sys::VolatilePointer;

/**
 Only even-numbered addresses in SRAM are usable.
 This pointer wrapper lets you access the even addresses of SRAM
 as if it were a contiguous memory region.
*/
#[repr(transparent)]
#[derive(Clone, Copy)]
pub struct SramPointer<T>(VolatilePointer<T>);

impl<T> core::fmt::Debug for SramPointer<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        self.0.fmt(f)
    }
}

impl<const N: usize> SramPointer<[u8; N]> {
    pub fn read_slice(self, slice: &mut [u8], start: usize) {
        assert!(start + slice.len() <= N);
        let offsets = start..start + slice.len();
        for (dst, offset) in slice.iter_mut().zip(offsets) {
            let src: VolatilePointer<u8> = unsafe { self.0.field(offset * 2) };
            *dst = src.read();
        }
    }
    pub fn write_slice(self, slice: &[u8], start: usize) {
        assert!(start + slice.len() <= N);
        for (src, offset) in slice.iter().zip(start..start + slice.len()) {
            let dst: VolatilePointer<u8> = unsafe { self.0.field(offset * 2) };
            dst.write(*src);
        }
    }

    pub const fn index(self, index: usize) -> VolatilePointer<u8> {
        assert!(index < N);
        unsafe { self.0.field(index * 2) }
    }
}

pub const SRAM: SramPointer<[u8; 8192]> =
    SramPointer(unsafe { VolatilePointer::from_address(0x06000000) });

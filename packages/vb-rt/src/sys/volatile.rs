#[repr(transparent)]
#[derive(Clone, Copy)]
pub struct VolatilePointer<T>(*mut T);

impl<T: Copy> VolatilePointer<T> {
    /// Construct a new volatile pointer from an address.
    ///
    /// # Safety
    ///
    /// The given address must be valid for reads and writes.
    pub const unsafe fn from_address(address: usize) -> Self {
        assert!(address % align_of::<T>() == 0);
        Self(address as *mut T)
    }

    pub fn read(self) -> T {
        // SAFETY: constructor guarantees that address is valid and aligned
        unsafe { self.0.read_volatile() }
    }

    pub fn write(self, val: T) {
        // SAFETY: constructor guarantees that address is valid and aligned
        unsafe { self.0.write_volatile(val) }
    }
}

impl<T: Copy, const N: usize> VolatilePointer<[T; N]> {
    pub fn write_slice(self, slice: &[T], start: usize) {
        assert!(start + slice.len() < N);
        for (src, offset) in slice.iter().zip(start..start + slice.len()) {
            unsafe { self.0.cast::<T>().add(offset).write_volatile(*src) };
        }
    }
}

macro_rules! mmio {
    () => {};
    (pub const $name:ident: $type:ty = $address:literal; $($rest:tt)*) => {
        pub const $name: $crate::sys::VolatilePointer<$type> = unsafe { $crate::sys::VolatilePointer::from_address($address) };
        mmio!($($rest)*);
    };
}
pub(crate) use mmio;

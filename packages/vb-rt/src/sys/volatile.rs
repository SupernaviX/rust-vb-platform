#[repr(transparent)]
#[derive(Clone, Copy)]
pub struct VolatilePointer<T>(*mut T);

impl<T> VolatilePointer<T> {
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

macro_rules! mmio {
    () => {};
    (pub const $name:ident: $type:ty = $address:literal; $($rest:tt)*) => {
        pub const $name: $crate::sys::VolatilePointer<$type> = unsafe { $crate::sys::VolatilePointer::from_address($address) };
        mmio!($($rest)*);
    };
}
pub(crate) use mmio;

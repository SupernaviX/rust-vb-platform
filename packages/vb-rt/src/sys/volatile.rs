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

    /// Construct a new volatile pointer to a field of the data at this address.
    ///
    /// # Safety
    ///
    /// The given offset must point to a field of the given type.
    pub const unsafe fn field<U: Copy>(self, offset: usize) -> VolatilePointer<U> {
        assert!(offset < size_of::<T>());
        let inner = unsafe { self.0.cast::<u8>().add(offset) }.cast::<U>();
        VolatilePointer(inner)
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
        assert!(start + slice.len() <= N);
        for (src, offset) in slice.iter().zip(start..start + slice.len()) {
            unsafe { self.0.cast::<T>().add(offset).write_volatile(*src) };
        }
    }

    pub const fn index(self, index: usize) -> VolatilePointer<T> {
        assert!(index < N);
        unsafe { VolatilePointer(self.0.cast::<T>().add(index)) }
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

macro_rules! mmstruct {
    (
        $(#[$struct_attr:meta])*
        $struct_vis:vis struct $struct_name:ident {
            $(
                $(#[$field_attr:meta])*
                $field_vis:vis $field:ident: $field_ty:ty,
            )*
        }
    ) => {
        $(#[$struct_attr])*
        $struct_vis struct $struct_name {
            $(
                $(#[$field_attr])*
                $field_vis $field: $field_ty,
            )*
        }

        impl $crate::sys::VolatilePointer<$struct_name> {
            $(
                $(#[$field_attr])*
                $field_vis const fn $field(self) -> $crate::sys::VolatilePointer<$field_ty> {
                    let offset = core::mem::offset_of!($struct_name, $field);
                    // SAFETY: this is definitely the offset of a field which exists on this type.
                    unsafe { self.field(offset) }
                }
            )*
        }
    };
}
pub(crate) use mmstruct;

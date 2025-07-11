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
    pub(crate) const unsafe fn field<U: Copy>(self, offset: usize) -> VolatilePointer<U> {
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

impl<T> core::fmt::Debug for VolatilePointer<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_fmt(format_args!("0x{:08x}", self.0.addr()))
    }
}

impl<T: Copy, const N: usize> VolatilePointer<[T; N]> {
    pub fn read_slice(self, slice: &mut [T], start: usize) {
        assert!(start + slice.len() <= N);
        let offsets = start..start + slice.len();
        for (dst, offset) in slice.iter_mut().zip(offsets) {
            *dst = unsafe { self.0.cast::<T>().add(offset).read_volatile() };
        }
    }

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

/**
Some MMIO regions (SRAM, waveforms) are only writable at specific alignments.
This pointer lets you interact with them as if they were a contiguous address space.
*/
#[repr(transparent)]
#[derive(Clone, Copy)]
pub struct OveralignedVolatilePointer<T, const A: usize>(VolatilePointer<T>);

impl<T, const A: usize> core::fmt::Debug for OveralignedVolatilePointer<T, A> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        self.0.fmt(f)
    }
}

impl<const N: usize, const A: usize> OveralignedVolatilePointer<[u8; N], A> {
    /// Construct a new volatile pointer from an address.
    ///
    /// # Safety
    ///
    /// The given address must be valid for reads and writes.
    pub const unsafe fn from_address(address: usize) -> Self {
        Self(unsafe { VolatilePointer::from_address(address) })
    }

    pub fn read_slice(self, slice: &mut [u8], start: usize) {
        assert!(start + slice.len() < N);
        let offsets = start..start + slice.len();
        for (dst, offset) in slice.iter_mut().zip(offsets) {
            let src: VolatilePointer<u8> = unsafe { self.0.field(offset * A) };
            *dst = src.read();
        }
    }

    pub fn write_slice(self, slice: &[u8], start: usize) {
        assert!(start + slice.len() <= N);
        for (src, offset) in slice.iter().zip(start..start + slice.len()) {
            let dst: VolatilePointer<u8> = unsafe { self.0.field(offset * A) };
            dst.write(*src);
        }
    }

    pub const fn index(self, index: usize) -> VolatilePointer<u8> {
        assert!(index < N);
        unsafe { self.0.field(index * A) }
    }
}

macro_rules! mmio {
    () => {};
    (
        $(#[$ptr_attr:meta])*
        $ptr_vis:vis const $name:ident: $type:ty = $address:literal; $($rest:tt)*
    ) => {
        $(#[$ptr_attr])*
        $ptr_vis const $name: $crate::sys::VolatilePointer<$type> = unsafe { $crate::sys::VolatilePointer::from_address($address) };
        mmio!($($rest)*);
    };
    (
        $(#[$ptr_attr:meta])*
        $ptr_vis:vis const $name:ident: $type:ty, align $align:literal = $address:literal; $($rest:tt)*
    ) => {
        $(#[$ptr_attr])*
        $ptr_vis const $name: $crate::sys::OveralignedVolatilePointer<$type, $align> = unsafe { $crate::sys::OveralignedVolatilePointer::from_address($address) };
        mmio!($($rest)*);
    }
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

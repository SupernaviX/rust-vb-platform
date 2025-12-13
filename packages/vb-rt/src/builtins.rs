// Expose any builtins from compiler-rt here.

unsafe extern "C" {
    #[link_name = "__memcpy_wordaligned"]
    pub unsafe fn memcpy_wordaligned(dest: *mut u8, src: *const u8, count: usize) -> *mut u8;
}

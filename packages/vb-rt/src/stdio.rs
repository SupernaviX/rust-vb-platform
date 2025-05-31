use core::fmt::Write;

use crate::sys;

pub struct OutWriter;

impl Write for OutWriter {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        for b in s.as_bytes() {
            self.write_byte(*b);
        }
        Ok(())
    }
}

impl OutWriter {
    #[inline]
    pub fn write_byte(&self, b: u8) {
        sys::hardware::STDOUT.write(b);
    }

    #[inline]
    pub fn write_nl(&self) {
        self.write_byte(b'\n');
    }
}

use arrayvec::ArrayVec;
use vb_rt::sys::vip;

use crate::Font;

pub struct TextRenderer {
    font: &'static Font,
    chardata_start: u16,
    chars: (u16, u16),
    chardata_index: u16,
    char_offset: (u8, u8),
}

impl TextRenderer {
    pub fn new(font: &'static Font, chardata_start: u16, chars: (u8, u8)) -> Self {
        Self {
            font,
            chardata_start,
            chars: (chars.0 as u16, chars.1 as u16),
            chardata_index: chardata_start,
            char_offset: (0, 0),
        }
    }

    pub fn render_to_bgmap(&self, index: u8, dst: (u8, u8)) -> (i16, i16) {
        let map = vip::BG_MAPS.index(index as usize);
        for y in 0..self.chars.1 {
            for x in 0..self.chars.0 {
                let character = self.chardata_start + (y * self.chars.0) + x;
                let cell = vip::Cell::new().with_character(character);

                let dst_y = dst.1 as usize + y as usize;
                let dst = (dst_y * 64) + (dst.0 as usize + x as usize);
                map.index(dst).write(cell);
            }
        }
        (dst.0 as i16 * 8, dst.1 as i16 * 8)
    }

    pub fn width(&self) -> i16 {
        let chars_drawn = self.chardata_index - self.chardata_start;
        if chars_drawn > self.chars.0 {
            self.chars.0 as i16 * 8
        } else {
            chars_drawn as i16 * 8 + self.char_offset.0 as i16
        }
    }

    pub fn clear(&mut self) {
        self.chardata_index = self.chardata_start;
        self.char_offset = (0, 0);
        for char_y in 0..self.chars.1 {
            let index = self.chardata_start + (char_y * self.chars.0);
            for row in 0..8 {
                erase_row(index, (0, row), self.chars.0 * 8);
            }
        }
    }

    pub fn is_empty(&self) -> bool {
        self.chardata_index == self.chardata_start && self.char_offset == (0, 0)
    }

    pub fn draw_text(&mut self, text: &[u8]) -> bool {
        for char in text {
            if !self.draw_char(*char) {
                return false;
            }
        }
        true
    }

    fn draw_char(&mut self, char: u8) -> bool {
        if char == b'\n' {
            let chardata_offset = self.chardata_index - self.chardata_start;
            self.chardata_index =
                self.chardata_start + chardata_offset - (chardata_offset % self.chars.0);
            self.char_offset.0 = 0;
            self.char_offset.1 += self.font.line_height as u8;
            while self.char_offset.1 >= 8 {
                self.char_offset.1 -= 8;
                self.chardata_index += self.chars.0;
            }
            return self.chardata_index < self.chardata_start + (self.chars.0 * self.chars.1);
        }
        let font_char_data = &self.font.chars[char as usize];
        let mut index = self.chardata_index;
        let (dst_x, mut dst_y) = self.char_offset;
        let y_top = font_char_data.y_offset;
        let y_bottom = y_top + font_char_data.height;
        for y in 0..self.font.line_height {
            if index > (self.chardata_start + self.chars.0 * self.chars.1) {
                if y < y_bottom {
                    return false;
                }
            } else if y >= y_top && y < y_bottom {
                self.font.texture.render_row_to_chardata(
                    index,
                    (dst_x, dst_y),
                    (font_char_data.x, y - y_top),
                    font_char_data.width + 1,
                );
            }
            dst_y += 1;
            if dst_y == 8 {
                dst_y = 0;
                index += self.chars.0;
            }
        }

        self.char_offset.0 += font_char_data.width as u8 + 1;
        while self.char_offset.0 >= 8 {
            self.char_offset.0 -= 8;
            self.chardata_index += 1;
            if ((self.chardata_index - self.chardata_start) % self.chars.0) == 0 {
                return false;
            }
        }
        true
    }

    pub fn buffered<const N: usize>(self, delay: u8) -> BufferedTextRenderer<N> {
        BufferedTextRenderer {
            buffer: ArrayVec::new(),
            buffer_index: 0,
            delay,
            counter: 0,
            inner: self,
        }
    }
}

fn erase_row(index: u16, dst: (u8, u8), size: u16) {
    let mut dst_addr = (index as usize * 8) + dst.1 as usize;
    let mut remaining = size;
    if remaining < 8 - dst.0 as u16 {
        let mask = ((1 << ((remaining as u8 + dst.0) * 2)) - 1) & !((1 << (dst.0 * 2)) - 1);
        let dest = vip::CHARACTER_HWS.index(dst_addr);
        dest.write(dest.read() & !mask);
        return;
    }
    if dst.0 > 0 {
        let mask = (1 << (dst.0 * 2)) - 1;
        let dest = vip::CHARACTER_HWS.index(dst_addr);
        dest.write(dest.read() & mask);
        remaining -= 8 - dst.0 as u16;
        dst_addr += 8;
    }
    while remaining >= 8 {
        let dest = vip::CHARACTER_HWS.index(dst_addr);
        dest.write(0);
        remaining -= 8;
        dst_addr += 8;
    }
    if remaining > 0 {
        let mask = (1 << (remaining * 2)) - 1;
        let dest = vip::CHARACTER_HWS.index(dst_addr);
        dest.write(dest.read() & !mask);
    }
}

impl core::fmt::Write for TextRenderer {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        if self.draw_text(s.as_bytes()) {
            Ok(())
        } else {
            Err(core::fmt::Error)
        }
    }
}

pub struct BufferedTextRenderer<const N: usize> {
    buffer: ArrayVec<u8, N>,
    buffer_index: usize,
    delay: u8,
    counter: u8,
    pub inner: TextRenderer,
}

impl<const N: usize> BufferedTextRenderer<N> {
    pub fn render_to_bgmap(&self, index: u8, dst: (u8, u8)) -> (i16, i16) {
        self.inner.render_to_bgmap(index, dst)
    }

    pub fn clear(&mut self) {
        self.buffer.clear();
        self.buffer_index = 0;
        self.counter = self.delay;
        self.inner.clear();
    }

    pub fn is_empty(&self) -> bool {
        self.buffer_index == self.buffer.len() && self.inner.is_empty()
    }

    pub fn width(&self) -> i16 {
        self.inner.width()
    }

    pub fn final_width(&self) -> i16 {
        self.width() + self.inner.font.measure(&self.buffer[self.buffer_index..]) as i16
    }

    pub fn draw_text(&mut self, text: &[u8]) -> bool {
        if self.buffer.remaining_capacity() < text.len() {
            return false;
        }
        self.buffer.extend(text.iter().copied());
        true
    }

    pub fn update(&mut self) -> bool {
        if self.counter < self.delay {
            self.counter += 1;
            false
        } else if self.buffer_index == self.buffer.len() {
            true
        } else {
            self.counter = 0;
            self.inner.draw_char(self.buffer[self.buffer_index]);
            self.buffer_index += 1;
            false
        }
    }
}

impl<const N: usize> core::fmt::Write for BufferedTextRenderer<N> {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        if self.draw_text(s.as_bytes()) {
            Ok(())
        } else {
            Err(core::fmt::Error)
        }
    }
}

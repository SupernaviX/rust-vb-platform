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
                let cell = vip::BGCell::new().with_character(character);

                let dst_y = dst.1 as usize + y as usize;
                let dst = (dst_y * 64) + (dst.0 as usize + x as usize);
                map.index(dst).write(cell);
            }
        }
        (dst.0 as i16 * 8, dst.1 as i16 * 8)
    }

    pub fn draw_text(&mut self, text: &[u8]) {
        for char in text {
            self.draw_char(*char);
        }
    }

    fn draw_char(&mut self, char: u8) {
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
            return;
        }
        let font_char_data = &self.font.chars[char as usize];
        let mut index = self.chardata_index;
        let (dst_x, mut dst_y) = self.char_offset;
        for y in 0..font_char_data.height {
            self.font.texture.render_row_to_chardata(
                index,
                (dst_x, dst_y),
                (font_char_data.x, font_char_data.y + y),
                font_char_data.width,
            );
            dst_y += 1;
            if dst_y == 8 {
                dst_y = 0;
                index += self.chars.0;
            }
        }

        self.char_offset.0 += font_char_data.width as u8;
        while self.char_offset.0 >= 8 {
            self.char_offset.0 -= 8;
            self.chardata_index += 1;
        }
    }
}

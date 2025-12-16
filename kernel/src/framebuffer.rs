use bootloader_api::info::{FrameBufferInfo, PixelFormat};
use core::fmt;
use font8x8::{BASIC_FONTS, UnicodeFonts};
use spin::Mutex;

// We need a global lock so we can print from interrupts/threads safely
pub static WRITER: Mutex<Option<FrameBufferWriter>> = Mutex::new(None);

pub struct FrameBufferWriter {
    buffer: &'static mut [u8],
    info: FrameBufferInfo,
    x_pos: usize,
    y_pos: usize,
    color: [u8; 3], // R, G, B
}

impl FrameBufferWriter {
    pub fn new(buffer: &'static mut [u8], info: FrameBufferInfo) -> Self {
        let mut writer = Self {
            buffer,
            info,
            x_pos: 0,
            y_pos: 0,
            color: [255, 255, 255], // Default to White
        };
        writer.clear();
        writer
    }

    pub fn set_color(&mut self, r: u8, g: u8, b: u8) {
        self.color = [r, g, b];
    }

    pub fn clear(&mut self) {
        self.x_pos = 0;
        self.y_pos = 0;
        self.buffer.fill(0);
    }

    pub fn backspace(&mut self) {
        if self.x_pos >= 8 {
            self.x_pos -= 8;
            // Overwrite the previous character with black (0,0,0)
            self.draw_rect(self.x_pos, self.y_pos, 8, 8, 0, 0, 0);
        } else if self.y_pos >= 8 {
            self.y_pos -= 8;
            self.x_pos = self.info.width - 8;
            self.draw_rect(self.x_pos, self.y_pos, 8, 8, 0, 0, 0);
        }
    }

    fn newline(&mut self) {
        self.x_pos = 0;
        self.y_pos += 8; // Move down 8 pixels (font height)

        // If we hit the bottom of the screen, scroll up
        if self.y_pos + 8 > self.info.height {
            self.scroll_up();
        }
    }

    fn scroll_up(&mut self) {
        let height = self.info.height;
        let stride = self.info.stride;
        let bpp = self.info.bytes_per_pixel;
        let font_height = 8;

        // 1. Calculate the size of one line of text in bytes
        let line_bytes = stride * bpp * font_height;
        let total_bytes = stride * bpp * height;

        // 2. Shift the entire buffer content UP by one line
        // copy_within(src_start..src_end, dest_start)
        self.buffer.copy_within(line_bytes..total_bytes, 0);

        // 3. Clear the last line (now duplicated) with black
        let last_line_start = total_bytes - line_bytes;

        // Bounds check to be safe
        if last_line_start < self.buffer.len() {
            self.buffer[last_line_start..].fill(0);
        }

        // 4. Reset y_pos to the start of the last line
        self.y_pos -= font_height;
    }

    // Helper to clear a specific area (used by backspace)
    fn draw_rect(
        &mut self,
        x_start: usize,
        y_start: usize,
        width: usize,
        height: usize,
        r: u8,
        g: u8,
        b: u8,
    ) {
        for y in y_start..y_start + height {
            for x in x_start..x_start + width {
                if x < self.info.width && y < self.info.height {
                    self.write_pixel(x, y, r, g, b);
                }
            }
        }
    }

    fn write_char(&mut self, c: char) {
        match c {
            '\n' => self.newline(),
            '\r' => self.x_pos = 0,     // Carriage return
            '\x08' => self.backspace(), // Backspace control char
            _ => {
                // If we are at the edge of the screen, move to next line
                if self.x_pos + 8 >= self.info.width {
                    self.newline();
                }

                // Draw the character using font8x8
                if let Some(bitmap) = BASIC_FONTS.get(c) {
                    for (row_i, row_byte) in bitmap.iter().enumerate() {
                        for col_i in 0..8 {
                            if *row_byte & (1 << col_i) != 0 {
                                self.write_pixel(
                                    self.x_pos + col_i,
                                    self.y_pos + row_i,
                                    self.color[0],
                                    self.color[1],
                                    self.color[2],
                                );
                            }
                        }
                    }
                }
                self.x_pos += 8;
            }
        }
    }

    fn write_pixel(&mut self, x: usize, y: usize, r: u8, g: u8, b: u8) {
        let pixel_offset = y * self.info.stride + x;
        let color = match self.info.pixel_format {
            PixelFormat::Rgb => [r, g, b, 0],
            PixelFormat::Bgr => [b, g, r, 0],
            PixelFormat::U8 => [if r > 128 { 0xff } else { 0 }, 0, 0, 0], // Greyscale fallback
            other => panic!("pixel format {:?} not supported in logger", other),
        };

        let bytes_per_pixel = self.info.bytes_per_pixel;
        let byte_offset = pixel_offset * bytes_per_pixel;

        // Bounds check
        if byte_offset + (bytes_per_pixel - 1) < self.buffer.len() {
            self.buffer[byte_offset..(byte_offset + bytes_per_pixel)]
                .copy_from_slice(&color[..bytes_per_pixel]);
        }
    }
}

// Implement fmt::Write so we can use the `write!` macro
impl fmt::Write for FrameBufferWriter {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for c in s.chars() {
            self.write_char(c);
        }
        Ok(())
    }
}

// Global Macros
#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => ($crate::framebuffer::_print(format_args!($($arg)*)));
}

#[macro_export]
macro_rules! println {
    () => ($crate::print!("\n"));
    ($($arg:tt)*) => ($crate::print!("{}\n", format_args!($($arg)*)));
}

#[doc(hidden)]
pub fn _print(args: fmt::Arguments) {
    use core::fmt::Write;
    use x86_64::instructions::interrupts;

    // Disable interrupts to avoid deadlock if an interrupt tries to print
    interrupts::without_interrupts(|| {
        if let Some(writer) = WRITER.lock().as_mut() {
            writer.write_fmt(args).unwrap();
        }
    });
}

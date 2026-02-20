use alloc::vec;
use alloc::vec::Vec;
use bootloader_api::info::{FrameBufferInfo, PixelFormat};
use core::cmp::{max, min};
use core::fmt;
use font8x8::{BASIC_FONTS, UnicodeFonts};
use spin::Mutex;

// Global lock
pub static WRITER: Mutex<Option<FrameBufferWriter>> = Mutex::new(None);

pub struct FrameBufferWriter {
    framebuffer: &'static mut [u8], // Slow VRAM (Write-only mostly)
    backbuffer: Vec<u8>,            // Fast RAM (Read/Write)
    info: FrameBufferInfo,
    x_pos: usize,
    y_pos: usize,
    // Pre-calculated color bytes for the specific pixel format (R,G,B,A/Pad)
    color_bytes: [u8; 4],
    scale: usize,
    // Optimization: Dirty Rectangle Tracking
    dirty_min_x: usize,
    dirty_min_y: usize,
    dirty_max_x: usize,
    dirty_max_y: usize,
}

impl FrameBufferWriter {
    pub fn new(framebuffer: &'static mut [u8], info: FrameBufferInfo) -> Self {
        // Initialize backbuffer with the same size as the framebuffer
        // We use RAM for drawing because reading from VRAM is extremely slow.
        let backbuffer = vec![0u8; framebuffer.len()];

        let mut writer = Self {
            framebuffer,
            backbuffer,
            info,
            x_pos: 0,
            y_pos: 0,
            color_bytes: [255, 255, 255, 0], // Default to white
            scale: 1,
            // Initialize dirty rect to the full screen so the first present() draws everything
            dirty_min_x: 0,
            dirty_min_y: 0,
            dirty_max_x: info.width,
            dirty_max_y: info.height,
        };

        // Calculate the correct byte order for white immediately
        writer.set_color(255, 255, 255);
        writer.clear();
        writer
    }

    /// Marks a region of the screen as "dirty" (needs to be copied to VRAM).
    fn mark_dirty(&mut self, x: usize, y: usize, width: usize, height: usize) {
        let screen_width = self.info.width;
        let screen_height = self.info.height;

        // Clamp input to screen bounds
        let start_x = min(x, screen_width);
        let start_y = min(y, screen_height);
        let end_x = min(x + width, screen_width);
        let end_y = min(y + height, screen_height);

        // Expand the dirty area to include this new rectangle
        self.dirty_min_x = min(self.dirty_min_x, start_x);
        self.dirty_min_y = min(self.dirty_min_y, start_y);
        self.dirty_max_x = max(self.dirty_max_x, end_x);
        self.dirty_max_y = max(self.dirty_max_y, end_y);
    }

    /// Flushes the backbuffer to the actual VRAM.
    /// This uses the dirty rectangle to copy ONLY the changed pixels.
    pub fn present(&mut self) {
        // If nothing is dirty, do nothing
        if self.dirty_min_x >= self.dirty_max_x || self.dirty_min_y >= self.dirty_max_y {
            return;
        }

        let stride = self.info.stride;
        let bpp = self.info.bytes_per_pixel;

        // We iterate row by row within the dirty Y range
        for y in self.dirty_min_y..self.dirty_max_y {
            let row_start = (y * stride) + self.dirty_min_x;
            let row_end = (y * stride) + self.dirty_max_x;

            let byte_start = row_start * bpp;
            let byte_end = row_end * bpp;

            // Safety check to ensure we don't go out of bounds
            if byte_end <= self.framebuffer.len() {
                self.framebuffer[byte_start..byte_end]
                    .copy_from_slice(&self.backbuffer[byte_start..byte_end]);
            }
        }

        // Reset dirty rect to "inverted" (empty) state
        self.dirty_min_x = self.info.width;
        self.dirty_min_y = self.info.height;
        self.dirty_max_x = 0;
        self.dirty_max_y = 0;
    }

    pub fn set_color(&mut self, r: u8, g: u8, b: u8) {
        // Pre-calculate the pixel bytes based on format so we don't check every pixel
        match self.info.pixel_format {
            PixelFormat::Rgb => self.color_bytes = [r, g, b, 0],
            PixelFormat::Bgr => self.color_bytes = [b, g, r, 0],
            PixelFormat::U8 => {
                let gray = if r > 128 { 0xff } else { 0 };
                self.color_bytes = [gray, gray, gray, 0];
            }
            _ => panic!("unsupported pixel format"),
        }
    }

    pub fn set_scale(&mut self, scale: usize) {
        if scale > 0 {
            self.scale = scale;
        }
    }

    pub fn clear(&mut self) {
        self.x_pos = 0;
        self.y_pos = 0;
        self.backbuffer.fill(0);

        // Mark the entire screen as dirty to ensure the black screen is drawn
        self.mark_dirty(0, 0, self.info.width, self.info.height);
        self.present();
    }

    pub fn width(&self) -> usize {
        self.info.width
    }

    pub fn height(&self) -> usize {
        self.info.height
    }

    fn backspace(&mut self) {
        let step = 8 * self.scale;
        if self.x_pos >= step {
            self.x_pos -= step;
            // Overwrite with black
            self.draw_rect(self.x_pos, self.y_pos, step, step, true);
        } else if self.y_pos >= step {
            self.y_pos -= step;
            self.x_pos = self.info.width - step;
            self.draw_rect(self.x_pos, self.y_pos, step, step, true);
        }
    }

    fn newline(&mut self) {
        let font_height = 8 * self.scale;
        self.x_pos = 0;
        self.y_pos += font_height;

        if self.y_pos + font_height > self.info.height {
            self.scroll_up();
        }
    }

    /// Optimized scroll using `copy_within` on the RAM backbuffer.
    fn scroll_up(&mut self) {
        let stride = self.info.stride;
        let bpp = self.info.bytes_per_pixel;
        let font_height = 8 * self.scale;
        let height = self.info.height;

        let line_bytes = stride * bpp * font_height;
        let total_bytes = stride * bpp * height;

        // 1. Shift data up in fast RAM
        self.backbuffer.copy_within(line_bytes..total_bytes, 0);

        // 2. Clear the bottom line
        let last_line_start = total_bytes - line_bytes;
        if last_line_start < self.backbuffer.len() {
            self.backbuffer[last_line_start..].fill(0);
        }

        self.y_pos -= font_height;

        // Scrolling invalidates the WHOLE screen, so we must redraw everything.
        self.mark_dirty(0, 0, self.info.width, self.info.height);
    }

    /// Optimized rectangle drawer.
    /// `is_clear`: if true, draws black. If false, draws current color.
    pub fn draw_rect(
        &mut self,
        x_start: usize,
        y_start: usize,
        width: usize,
        height: usize,
        is_clear: bool,
    ) {
        let bpp = self.info.bytes_per_pixel;
        let stride = self.info.stride;
        let screen_width = self.info.width;
        let screen_height = self.info.height;

        // Clip to screen bounds to prevent panics
        let draw_width = if x_start + width > screen_width {
            screen_width - x_start
        } else {
            width
        };
        let draw_height = if y_start + height > screen_height {
            screen_height - y_start
        } else {
            height
        };

        // Determine color to write
        let pixel_bytes = if is_clear {
            &[0, 0, 0, 0][..bpp]
        } else {
            &self.color_bytes[..bpp]
        };

        // Draw row by row into BACKBUFFER
        for y in 0..draw_height {
            let row_idx = y_start + y;
            // Calculate the starting byte index for this row
            let row_start_offset = row_idx * stride * bpp;
            let pixel_offset = row_start_offset + (x_start * bpp);

            // Draw the pixels for this row
            for x in 0..draw_width {
                let offset = pixel_offset + (x * bpp);
                if offset + bpp <= self.backbuffer.len() {
                    self.backbuffer[offset..offset + bpp].copy_from_slice(pixel_bytes);
                }
            }
        }

        // Register the change so present() knows to draw it
        self.mark_dirty(x_start, y_start, draw_width, draw_height);
    }

    fn write_char(&mut self, c: char) {
        match c {
            '\n' => self.newline(),
            '\r' => self.x_pos = 0,
            '\x08' => self.backspace(),
            _ => {
                let scale = self.scale;
                let char_width = 8 * scale;

                if self.x_pos + char_width >= self.info.width {
                    self.newline();
                }

                if let Some(bitmap) = BASIC_FONTS.get(c) {
                    for (row_i, row_byte) in bitmap.iter().enumerate() {
                        for col_i in 0..8 {
                            if *row_byte & (1 << col_i) != 0 {
                                self.draw_rect(
                                    self.x_pos + col_i * scale,
                                    self.y_pos + row_i * scale,
                                    scale,
                                    scale,
                                    false, // Draw color
                                );
                            }
                        }
                    }
                }
                self.x_pos += char_width;
            }
        }
    }
}

// Implement fmt::Write
impl fmt::Write for FrameBufferWriter {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for c in s.chars() {
            self.write_char(c);
        }
        // Important: We only flush to VRAM once per print!
        // This effectively batches the drawing operations.
        self.present();
        Ok(())
    }
}

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

    interrupts::without_interrupts(|| {
        if let Some(writer) = WRITER.lock().as_mut() {
            writer.write_fmt(args).unwrap();
        }
    });
}

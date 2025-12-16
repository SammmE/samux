#![no_std]
#![no_main]

extern crate alloc;

use bootloader_api::{BootInfo, BootloaderConfig, config::Mapping, entry_point};
use font8x8::{BASIC_FONTS, UnicodeFonts};
use x86_64::VirtAddr;
use x86_64::instructions::hlt;

use kernel::allocator;
use kernel::init_all;
use kernel::memory::{self, BootInfoFrameAllocator};
use kernel::serial_println;

use alloc::{boxed::Box, rc::Rc, vec, vec::Vec};

pub static BOOTLOADER_CONFIG: BootloaderConfig = {
    let mut config = BootloaderConfig::new_default();
    config.mappings.physical_memory = Some(Mapping::Dynamic);
    config
};

entry_point!(kernel_main, config = &BOOTLOADER_CONFIG);

fn kernel_main(boot_info: &'static mut BootInfo) -> ! {
    serial_println!("Kernel initialized successfully!\n");
    init_all();
    serial_println!("IDT initialized.\n");

    let phys_mem_offset = VirtAddr::new(boot_info.physical_memory_offset.into_option().unwrap());
    let mut mapper = unsafe { memory::init(phys_mem_offset) };
    let mut frame_allocator = unsafe { BootInfoFrameAllocator::init(&boot_info.memory_regions) };

    allocator::init_heap(&mut mapper, &mut frame_allocator).expect("heap initialization failed");

    // --- HEAP TEST ---
    let heap_value = Box::new(41);
    serial_println!("Heap value at {:p}", heap_value);

    let mut vec = Vec::new();
    for i in 0..500 {
        vec.push(i);
    }
    serial_println!("Vec at {:p}", vec.as_slice());
    // ------------------------------------

    if let Some(framebuffer) = boot_info.framebuffer.as_mut() {
        let info = framebuffer.info();
        let bytes_per_pixel = info.bytes_per_pixel;
        let stride = info.stride;
        let buffer = framebuffer.buffer_mut();

        for pixel in buffer.chunks_exact_mut(bytes_per_pixel) {
            pixel[0] = 0; // Blueish
            pixel[1] = 0; // Greenish
            pixel[2] = 0; // Reddish
        }

        let message = "Hello World!";
        let mut x_pos = 100; // Start 100 pixels from the left
        let y_pos = 100; // Start 100 pixels from the top

        for char in message.chars() {
            // Draw the character at the current position
            draw_char(buffer, stride, bytes_per_pixel, x_pos, y_pos, char);

            // Move the cursor 8 pixels to the right for the next letter
            x_pos += 8;
        }
    }

    loop {
        hlt();
    }
}

/// Draws a single character to the framebuffer
fn draw_char(
    buffer: &mut [u8],
    stride: usize,
    bytes_per_pixel: usize,
    x: usize,
    y: usize,
    char: char,
) {
    if let Some(bitmap) = BASIC_FONTS.get(char) {
        for (row_i, row_byte) in bitmap.iter().enumerate() {
            for col_i in 0..8 {
                if *row_byte & (1 << col_i) != 0 {
                    let pixel_index = ((y + row_i) * stride + (x + col_i)) * bytes_per_pixel;

                    if pixel_index + 2 < buffer.len() {
                        buffer[pixel_index] = 255; // Blue
                        buffer[pixel_index + 1] = 255; // Green
                        buffer[pixel_index + 2] = 255; // Red
                    }
                }
            }
        }
    }
}

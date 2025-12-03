#![no_std] // don't link the Rust standard library
#![no_main] // disable all Rust-level entry points

use bootloader_api::{BootInfo, entry_point};
use core::fmt::Write;
use x86_64::instructions::{nop, port::Port, hlt};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum QemuExitCode {
    Success = 0x10,
    Failed = 0x11,
}

pub fn exit_qemu(exit_code: QemuExitCode) -> ! {
    unsafe {
        let mut port = Port::new(0xf4);
        port.write(exit_code as u32);
    }

    loop {
        nop();
    }
}

pub fn serial() -> uart_16550::SerialPort {
    let mut port = unsafe { uart_16550::SerialPort::new(0x3F8) };
    port.init();
    port
}

entry_point!(kernel_main);

static HELLO: &[u8] = b"Hello World!";

fn kernel_main(boot_info: &'static mut BootInfo) -> ! {
    let mut port = serial();
    writeln!(port, "Entered kernel with boot info: {boot_info:?}").unwrap();
    writeln!(port, "\nhello, world\n").unwrap();

    // NOTE: The VGA text buffer at 0xb8000 is not available!
    if let Some(framebuffer) = boot_info.framebuffer.as_mut() {
        let info = framebuffer.info();
        let buffer = framebuffer.buffer_mut();

        // 2. Iterate over every pixel on the screen
        // The buffer is just a long slice of u8 bytes.
        // We jump by 'bytes_per_pixel' (which is 3) to get to the next pixel.
        for pixel in buffer.chunks_exact_mut(info.bytes_per_pixel) {
            // 3. Write the color. Format is BGR (Blue, Green, Red) based on your logs.
            pixel[0] = 255; // Blue
            pixel[1] = 0; // Green
            pixel[2] = 0; // Red
        }
    }

    loop {
        hlt();
    }
}

/// This function is called on panic.
#[panic_handler]
#[cfg(not(test))]
fn panic(info: &core::panic::PanicInfo) -> ! {
    let _ = writeln!(serial(), "PANIC: {info}");
    exit_qemu(QemuExitCode::Failed);
}

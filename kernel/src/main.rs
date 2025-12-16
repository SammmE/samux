#![no_std]
#![no_main]

extern crate alloc;

use bootloader_api::{BootInfo, BootloaderConfig, config::Mapping, entry_point};
use font8x8::{BASIC_FONTS, UnicodeFonts};
use futures_util::stream::StreamExt;
use pc_keyboard::{DecodedKey, HandleControl, Keyboard, ScancodeSet1, layouts};
use x86_64::VirtAddr;
use x86_64::instructions::hlt;

use kernel::allocator;
use kernel::framebuffer::{self, WRITER};
use kernel::init_all;
use kernel::memory::{self, BootInfoFrameAllocator};
use kernel::serial_println;
use kernel::task::keyboard::ScancodeStream;
use kernel::task::{Task, simple_executor::SimpleExecutor};
use kernel::{print, println};

use alloc::{boxed::Box, rc::Rc, vec, vec::Vec};

pub static BOOTLOADER_CONFIG: BootloaderConfig = {
    let mut config = BootloaderConfig::new_default();
    config.mappings.physical_memory = Some(Mapping::Dynamic);
    config
};

entry_point!(kernel_main, config = &BOOTLOADER_CONFIG);

async fn async_number() -> u32 {
    42
}

async fn example_task() {
    let number = async_number().await;
    println!("Async number: {}", number);
}

async fn print_keypresses() {
    let mut scancode_stream = ScancodeStream::new();

    // We move the Keyboard state machine here, into the async task
    let mut keyboard = Keyboard::new(
        ScancodeSet1::new(),
        layouts::Us104Key,
        HandleControl::Ignore,
    );

    // This loop yields (sleeps) when the stream returns Poll::Pending
    while let Some(scancode) = scancode_stream.next().await {
        if let Ok(Some(key_event)) = keyboard.add_byte(scancode) {
            if let Some(key) = keyboard.process_keyevent(key_event) {
                match key {
                    DecodedKey::Unicode(character) => print!("{}", character),
                    DecodedKey::RawKey(key) => print!("{:?}", key),
                }
            }
        }
    }
}

fn kernel_main(boot_info: &'static mut BootInfo) -> ! {
    serial_println!("Kernel initialized successfully!\n");
    init_all();
    serial_println!("IDT initialized.\n");

    let phys_mem_offset = VirtAddr::new(boot_info.physical_memory_offset.into_option().unwrap());
    let mut mapper = unsafe { memory::init(phys_mem_offset) };
    let mut frame_allocator = unsafe { BootInfoFrameAllocator::init(&boot_info.memory_regions) };

    allocator::init_heap(&mut mapper, &mut frame_allocator).expect("heap initialization failed");

    if let Some(framebuffer) = boot_info.framebuffer.as_mut() {
        let info = framebuffer.info();
        let buffer = framebuffer.buffer_mut();

        // Lock the global writer and initialize it
        let mut writer = WRITER.lock();
        *writer = Some(framebuffer::FrameBufferWriter::new(buffer, info));
    }

    println!("Hello World from the Framebuffer!");
    println!("The heap value is: {:?}", Box::new(42));

    let mut executor = SimpleExecutor::new();
    executor.spawn(Task::new(print_keypresses()));
    executor.spawn(Task::new(example_task()));
    executor.run(); // This will loop indefinitely polling tasks

    loop {
        hlt();
    }
}

#![no_std]
#![no_main]

extern crate alloc;

use bootloader_api::{BootInfo, BootloaderConfig, config::Mapping, entry_point};
use x86_64::VirtAddr;
use x86_64::structures::paging::{FrameAllocator, Mapper, Page, PageTableFlags, Size4KiB};

use kernel::allocator;
use kernel::framebuffer::{self, WRITER};
use kernel::init_all;
use kernel::memory::{self, BootInfoFrameAllocator};
use kernel::println;
use kernel::serial_println;
use kernel::shell;
use kernel::task::{Task, executor::Executor};

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

    // USERSPACE TEST

    let user_code_page = Page::containing_address(VirtAddr::new(0x1000_0000));
    let user_stack_page = Page::containing_address(VirtAddr::new(0x2000_0000));
    let flags =
        PageTableFlags::PRESENT | PageTableFlags::WRITABLE | PageTableFlags::USER_ACCESSIBLE;

    unsafe {
        mapper
            .map_to(user_code_page, code_frame, flags, &mut frame_allocator)
            .unwrap()
            .flush();
        mapper
            .map_to(user_stack_page, stack_frame, flags, &mut frame_allocator)
            .unwrap()
            .flush();
    }

    let shellcode: [u8; 11] = [
        0x48, 0xC7, 0xC7, 0x34, 0x12, 0x00, 0x00, 0x0F, 0x05, 0xEB, 0xFE,
    ];

    unsafe {
        let dest = user_code_page.start_address().as_mut_ptr::<u8>();
        core::ptr::copy_nonoverlapping(shellcode.as_ptr(), dest, shellcode.len());
    }

    println!("Jumping to Userspace...");
    serial_println!("Jumping to Userspace...");

    unsafe {
        kernel::syscall::enter_userspace(
            user_code_page.start_address().as_u64(),
            user_stack_page.start_address().as_u64() + 4096u64,
        );
    }

    if let Some(framebuffer) = boot_info.framebuffer.as_mut() {
        let info = framebuffer.info();
        let buffer = framebuffer.buffer_mut();
        let mut writer = WRITER.lock();
        *writer = Some(framebuffer::FrameBufferWriter::new(buffer, info));
    }

    println!("Hello World from the Framebuffer!");

    if let Some(writer) = WRITER.lock().as_mut() {
        writer.set_color(0, 255, 0); // Green
    }
    println!("This should be green");

    if let Some(writer) = WRITER.lock().as_mut() {
        writer.set_color(255, 255, 255); // Reset to white
    }

    let executor = Executor::new();
    executor.spawn(Task::new(shell::runshell()));

    executor.run();
}

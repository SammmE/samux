use crate::serial::{QemuExitCode, exit_qemu};
use crate::serial_println;

#[panic_handler]
#[cfg(not(test))]
pub fn panic(info: &core::panic::PanicInfo) -> ! {
    serial_println!("PANIC: {}", info);
    exit_qemu(QemuExitCode::Failed);
}

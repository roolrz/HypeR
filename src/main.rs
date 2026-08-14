#![no_std]
#![no_main]

extern crate alloc;

mod arch;
pub mod kernel;

use core::panic::PanicInfo;

#[panic_handler]
fn panic(info: &PanicInfo<'_>) -> ! {
    crate::kernel::log::panic(info);
    crate::arch::halt()
}

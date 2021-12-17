#![feature(alloc_error_handler)]
#![feature(const_fn)]
#![feature(decl_macro)]
#![feature(asm)]
#![feature(global_asm)]
#![feature(optin_builtin_traits)]
#![feature(raw_vec_internals)]
// ASHWIN added!
#![feature(slice_patterns)]
#![feature(panic_info_message)]
#![cfg_attr(not(test), no_std)]
#![cfg_attr(not(test), no_main)]

#[cfg(not(test))]
mod init;

extern crate alloc;

use crate::console::{kprint, kprintln};

pub mod allocator;
pub mod console;
pub mod fs;
pub mod mutex;
pub mod shell;

extern crate fat32;

extern crate pi;
use pi::uart;

#[inline(never)]
fn spin_sleep_ms(ms: usize) {
    for _ in 0..(ms * 6000) {
        unsafe {
            asm!("nop");
        }
    }
}

fn uart_loop() -> ! {
    let mut uart = uart::MiniUart::new();

    loop {
        let b = uart.read_byte();
        if b == 0x7f {
            uart.write_byte(0x08);
            uart.write_byte(b' ');
            uart.write_byte(0x08);
        } else {
            uart.write_byte(b);
        }
    }
}

use allocator::Allocator;
use fs::FileSystem;

#[cfg_attr(not(test), global_allocator)]
pub static ALLOCATOR: Allocator = Allocator::uninitialized();
pub static FILESYSTEM: FileSystem = FileSystem::uninitialized();

fn kmain() -> ! {
    // spin a tiny amount so we have time to connect our terminal to the UART
    spin_sleep_ms(100);

    kprintln!("===== Testing ATAGS ======");
    for tag in pi::atags::Atags::get() {
        kprintln!("{:#?}", tag);
    }
    unsafe {
        ALLOCATOR.initialize();
        FILESYSTEM.initialize();
    }

    kprintln!("===== Testing the allocator ======");
    use alloc::vec::Vec;
    let mut v = Vec::new();
    for i in 0..10 {
        v.push(i);
        kprintln!("{:?}", v);
    }

    kprintln!("Welcome to cs3210!");
    shell::shell(">");
}

#![feature(alloc_error_handler)]
#![feature(const_fn)]
#![feature(decl_macro)]
#![feature(asm)]
#![feature(global_asm)]
#![feature(optin_builtin_traits)]
#![feature(raw_vec_internals)]
#![cfg_attr(not(test), no_std)]
#![cfg_attr(not(test), no_main)]

#[cfg(not(test))]
mod init;

extern crate alloc;

pub mod allocator;
pub mod console;
pub mod fs;
pub mod mutex;
pub mod shell;

extern crate pi;
use pi::uart;

#[inline(never)]
fn spin_sleep_ms(ms: usize) {
    for _ in 0..(ms * 6000) {
        unsafe { asm!("nop"); }
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
    unsafe {
        ALLOCATOR.initialize();
        FILESYSTEM.initialize();
    }

    kprintln!("Welcome to cs3210!");
    shell::shell(">");
}

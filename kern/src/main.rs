#![feature(alloc_error_handler)]
#![feature(const_fn)]
#![feature(decl_macro)]
#![feature(asm)]
#![feature(global_asm)]
#![feature(optin_builtin_traits)]
#![cfg_attr(not(test), no_std)]
#![cfg_attr(not(test), no_main)]

#[cfg(not(test))]
mod init;

pub mod console;
pub mod mutex;
pub mod shell;

use core::time::Duration;

extern crate pi;
use pi::timer;
use pi::uart;

use console::kprintln;

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
        // kprint!("{:#x}", b);
        if b == 0x7f {
            uart.write_byte(0x08);
            uart.write_byte(b' ');
            uart.write_byte(0x08);
        } else {
            uart.write_byte(b);
        }
        // kprint!("{} {:#x}", b as char, b);
        // kprintln!(" <>");
        // uart.write_str("<-\n").unwrap();
    }
}

// FIXME: You need to add dependencies here to
// test your drivers (Phase 2). Add them as needed.

#[no_mangle]
pub unsafe extern "C" fn kmain() -> ! {
    // uart_loop();
    loop {
        kprintln!("{}\r\n", "hi");
        spin_sleep_ms(500);
    }
}

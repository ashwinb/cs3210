#![feature(alloc_error_handler)]
#![feature(const_fn)]
#![feature(decl_macro)]
#![feature(asm)]
#![feature(global_asm)]
#![feature(optin_builtin_traits)]
#![feature(ptr_internals)]
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
pub mod param;
pub mod process;
pub mod shell;
pub mod traps;
pub mod vm;

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
use process::GlobalScheduler;
use traps::irq::Irq;
use vm::VMManager;

#[cfg_attr(not(test), global_allocator)]
pub static ALLOCATOR: Allocator = Allocator::uninitialized();
pub static FILESYSTEM: FileSystem = FileSystem::uninitialized();
pub static SCHEDULER: GlobalScheduler = GlobalScheduler::uninitialized();
pub static VMM: VMManager = VMManager::uninitialized();
pub static IRQ: Irq = Irq::uninitialized();

fn kmain() -> ! {
    // spin a tiny amount so we have time to connect our terminal to the UART
    spin_sleep_ms(100);

    // kprintln!("===== Testing ATAGS ======");
    // for tag in pi::atags::Atags::get() {
    //     kprintln!("{:#?}", tag);
    // }
    unsafe {
        ALLOCATOR.initialize();
        FILESYSTEM.initialize();
        IRQ.initialize();
        kprintln!("initializing VMM");
        VMM.initialize();
        kprintln!("initializing scheduler");
        SCHEDULER.initialize();
        kprintln!("starting scheduler");
        SCHEDULER.start();
    }

    // kprintln!("===== Testing the allocator ======");
    // use alloc::vec::Vec;
    // let mut v = Vec::new();
    // for i in 0..10 {
    //     v.push(i);
    //     kprintln!("{:?}", v);
    // }

    // kprintln!("===== Testing exception levels =====");
    // unsafe {
    //     kprintln!("Current exception level = {}", aarch64::current_el());
    // }

    // kprintln!("Welcome to cs3210!");
    // // TODO: figure out how to recover from a DataAbort exception
    // // unsafe { asm!("b 0x450" :::: "volatile");}
    // aarch64::svc!(3);
    // aarch64::brk!(12);

    // loop {
    //     shell::shell(">");
    // }
}

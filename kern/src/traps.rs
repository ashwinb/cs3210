mod frame;
mod syndrome;
mod syscall;

pub mod irq;
pub use self::frame::TrapFrame;

use pi::interrupt::{Controller, Interrupt};

use self::syndrome::Syndrome;
use self::syscall::handle_syscall;
use crate::console::kprintln;
use crate::IRQ;
use aarch64::*;

#[repr(u16)]
#[derive(Debug, PartialEq, Eq, Copy, Clone)]
pub enum Kind {
    Synchronous = 0,
    Irq = 1,
    Fiq = 2,
    SError = 3,
}

#[repr(u16)]
#[derive(Debug, PartialEq, Eq, Copy, Clone)]
pub enum Source {
    CurrentSpEl0 = 0,
    CurrentSpElx = 1,
    LowerAArch64 = 2,
    LowerAArch32 = 3,
}

#[repr(C)]
#[derive(Debug, PartialEq, Eq, Copy, Clone)]
pub struct Info {
    source: Source,
    kind: Kind,
}

/// This function is called when an exception occurs. The `info` parameter
/// specifies the source and kind of exception that has occurred. The `esr` is
/// the value of the exception syndrome register. Finally, `tf` is a pointer to
/// the trap frame for the exception.
#[no_mangle]
pub extern "C" fn handle_exception(info: Info, esr: u32, tf: &mut TrapFrame) {
    // kprintln!("_________ exception handler {:?} ________", info);
    match info.kind {
        Kind::Synchronous => {
            let syn = Syndrome::from(esr);
            // kprintln!("esr: {}, syndrome: {:?}", esr, syn);
            match syn {
                Syndrome::Brk(_) => {
                    crate::shell::shell("debug =>");
                    tf.elr += 4;
                },
                Syndrome::Svc(n) => {
                    // kprintln!("handling syscall {:?}", syn);
                    handle_syscall(n, tf);
                },
                _ => {
                    kprintln!("Syndrome {:#?} not handled, FAR: {:#0x}", syn, unsafe { FAR_EL1.get() });
                }
            }
        },
        Kind::Irq => {
            let controller = Controller::new();
            for &int in Interrupt::iter() {
                if controller.is_pending(int) {
                    IRQ.invoke(int, tf);
                }
            }
        },
        _ => {
            kprintln!("Interrupt not handled: {:?}", info.kind);
        }
    }
    // kprintln!("________ handler returns _________");
}

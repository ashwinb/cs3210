use crate::common::IO_BASE;

use volatile::prelude::*;
use volatile::{Volatile, ReadVolatile, Reserved};

const INT_BASE: usize = IO_BASE + 0xB000 + 0x200;

#[derive(Copy, Clone, PartialEq)]
pub enum Interrupt {
    Timer1 = 1,
    Timer3 = 3,
    Usb = 9,
    Gpio0 = 49,
    Gpio1 = 50,
    Gpio2 = 51,
    Gpio3 = 52,
    Uart = 57,
}

impl Interrupt {
    pub const MAX: usize = 8;

    pub fn iter() -> core::slice::Iter<'static, Interrupt> {
        use Interrupt::*;
        [Timer1, Timer3, Usb, Gpio0, Gpio1, Gpio2, Gpio3, Uart].into_iter()
    }

    pub fn to_index(&self) -> usize {
        use Interrupt::*;
        match self {
            Timer1 => 0,
            Timer3 => 1,
            Usb => 2,
            Gpio0 => 3,
            Gpio1 => 4,
            Gpio2 => 5,
            Gpio3 => 6,
            Uart => 7,
        }
    }

    pub fn from_index(i: usize) -> Interrupt {
        use Interrupt::*;
        match i {
            0 => Timer1,
            1 => Timer3,
            2 => Usb,
            3 => Gpio0,
            4 => Gpio1,
            5 => Gpio2,
            6 => Gpio3,
            7 => Uart,
            _ => panic!("Unknown interrupt: {}", i),
        }
    }
}


impl From<usize> for Interrupt {
    fn from(irq: usize) -> Interrupt {
        use Interrupt::*;
        match irq {
            1 => Timer1,
            3 => Timer3,
            9 => Usb,
            49 => Gpio0,
            50 => Gpio1,
            51 => Gpio2,
            52 => Gpio3,
            57 => Uart,
            _ => panic!("Unkonwn irq: {}", irq),
        }
    }
}

// (C, packed) is dangerous especially if you use the Volatile constructs because
// you end up taking borrows to packed fields. Things go badly wrong with alignment
// Q: how do you ensure the repr() is correct?
// A: print address!
#[repr(C)]
#[allow(non_snake_case)]
struct Registers {
    // FIXME: Fill me in.
    BASIC_IRQ_PENDING: ReadVolatile<u32>,
    IRQ_PENDING: [ReadVolatile<u32>; 2],
    FIQ_CTRL: ReadVolatile<u8>,
    _r0: Reserved<[u8; 3]>,
    IRQ_ENABLE: [Volatile<u32>; 2],
    BASIC_IRQ_ENABLE: Volatile<u8>,
    _r1: Reserved<[u8; 3]>,
    IRQ_DISABLE: [Volatile<u32>; 2],
    BASIC_IRQ_DISABLE: Volatile<u8>,
    _r2: Reserved<[u8; 3]>,
}

/// An interrupt controller. Used to enable and disable interrupts as well as to
/// check if an interrupt is pending.
pub struct Controller {
    registers: &'static mut Registers
}

fn index(int: Interrupt) -> (usize, u8) {
    let val = int as u8;
    if val < 32 {
        (0, val)
    } else {
        (1, val - 32)
    }
}

impl Controller {
    /// Returns a new handle to the interrupt controller.
    pub fn new() -> Controller {
        Controller {
            registers: unsafe { &mut *(INT_BASE as *mut Registers) },
        }
    }

    pub fn current_enables(&self) -> (u32, u32) {
        (self.registers.IRQ_ENABLE[0].read(), self.registers.IRQ_ENABLE[1].read())
    }

    pub fn addr_of_enable_register(&self) -> usize {
        &self.registers.IRQ_ENABLE as *const Volatile<u32> as usize
    }

    /// Enables the interrupt `int`.
    pub fn enable(&mut self, int: Interrupt) {
        let (idx, bit) = index(int);
        self.registers.IRQ_ENABLE[idx].or_mask(0x1 << bit);
    }

    /// Disables the interrupt `int`.
    pub fn disable(&mut self, int: Interrupt) {
        let (idx, bit) = index(int);
        self.registers.IRQ_DISABLE[idx].or_mask(0x1 << bit);
    }

    /// Returns `true` if `int` is pending. Otherwise, returns `false`.
    pub fn is_pending(&self, int: Interrupt) -> bool {
        let (idx, bit) = index(int);
        self.registers.IRQ_PENDING[idx].has_mask(0x1 << bit)
    }
}

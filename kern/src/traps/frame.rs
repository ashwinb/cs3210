use core::fmt;

#[repr(C)]
#[derive(Default, Copy, Clone)]
pub struct TrapFrame {
    // FIXME: Fill me in.
    pub ttbr: [u64; 2],
    pub elr: u64,
    pub spsr: u64,
    pub sp: u64,
    pub tpidr: u64,
    pub qregs: [u128; 32],
    pub xregs: [u64; 32],
}

impl fmt::Debug for TrapFrame {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "xregs: ")?;
        for reg in self.xregs[0..7].iter() {
            write!(f, "{:#0x} ", *reg)?;
        }
        Ok(())
    }
}

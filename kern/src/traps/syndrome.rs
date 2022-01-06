use aarch64::ESR_EL1;

#[derive(Debug, PartialEq, Copy, Clone)]
pub enum Fault {
    AddressSize,
    Translation,
    AccessFlag,
    Permission,
    Alignment,
    TlbConflict,
    Other(u8),
}

impl From<u32> for Fault {
    fn from(val: u32) -> Fault {
        use Fault::*;

        let status = (val & 0xff) as u8;
        match status {
            0b0000..=0b0011 => AddressSize,
            0b0100..=0b0111 => Translation,
            0b1001..=0b1011 => AccessFlag,
            0b1101..=0b1111 => Permission,
            0b100_001 => Alignment,
            0b110_000 => TlbConflict,
            _ => Other(status),
        }
    }
}

fn fault_level(val: u32) -> u8 {
    (val & 0b11) as u8
}

#[derive(Debug, PartialEq, Copy, Clone)]
pub enum Syndrome {
    Unknown,
    WfiWfe,
    SimdFp,
    IllegalExecutionState,
    Svc(u16),
    Hvc(u16),
    Smc(u16),
    MsrMrsSystem,
    InstructionAbort { kind: Fault, level: u8 },
    PCAlignmentFault,
    DataAbort { kind: Fault, level: u8 },
    SpAlignmentFault,
    TrappedFpu,
    SError,
    Breakpoint,
    Step,
    Watchpoint,
    Brk(u16),
    Other(u32),
}

/// Converts a raw syndrome value (ESR) into a `Syndrome` (ref: D1.10.4).
impl From<u32> for Syndrome {
    fn from(esr_32: u32) -> Syndrome {
        use self::Syndrome::*;

        let esr = esr_32 as u64;
        let exc_class = ESR_EL1::get_value(esr, ESR_EL1::EC) as u32;
        let specific_syndrome = ESR_EL1::get_value(esr, ESR_EL1::ISS) as u32;

        match exc_class {
            0x0 => Unknown,
            0b1 => WfiWfe,
            0b00_0111 => SimdFp,
            0b00_1110 => IllegalExecutionState,
            // relying on fact that ::ISS_HSVC_IMM is the lowest 16 bits
            0b01_0101 => Svc(ESR_EL1::get_value(esr, ESR_EL1::ISS_HSVC_IMM) as u16),
            0b01_0110 => Hvc(ESR_EL1::get_value(esr, ESR_EL1::ISS_HSVC_IMM) as u16),
            0b01_0111 => Smc(ESR_EL1::get_value(esr, ESR_EL1::ISS_HSVC_IMM) as u16),
            0b01_1000 => MsrMrsSystem,
            0b10_0000 | 0b1_00001 => InstructionAbort {
                kind: specific_syndrome.into(),
                level: fault_level(specific_syndrome),
            },
            0b10_0010 => PCAlignmentFault,
            0b10_0100 | 0b1_00101 => DataAbort {
                kind: specific_syndrome.into(),
                level: fault_level(specific_syndrome),
            },
            0b10_0110 => SpAlignmentFault,
            0b10_1100 => TrappedFpu,
            0b11_0000 | 0b11_0001 => Breakpoint,
            0b11_0010 | 0b11_0011 => Step,
            0b11_0100 | 0b11_0101 => Watchpoint,
            0b11_1100 => Brk(ESR_EL1::get_value(esr, ESR_EL1::ISS_BRK_CMMT) as u16),
            _ => Other(esr_32),
        }
    }
}

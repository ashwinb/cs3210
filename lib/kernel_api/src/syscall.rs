use core::fmt;
use core::fmt::Write;
use core::time::Duration;

use crate::*;

macro_rules! err_or {
    ($ecode:expr, $rtn:expr) => {{
        let e = OsError::from($ecode);
        if let OsError::Ok = e {
            Ok($rtn)
        } else {
            Err(e)
        }
    }};
}

pub fn sleep(span: Duration) -> OsResult<Duration> {
    if span.as_millis() > core::u64::MAX as u128 {
        return Err(OsError::InvalidArgument);
    }

    let ms = span.as_millis() as u64;
    let mut ecode: u64;
    let mut elapsed_ms: u64;

    unsafe {
        asm!("mov x0, $2
              svc $3
              mov $0, x0
              mov $1, x7"
             : "=r"(elapsed_ms), "=r"(ecode)
             : "r"(ms), "i"(NR_SLEEP)
             : "x0", "x7"
             : "volatile");
    }

    err_or!(ecode, Duration::from_millis(elapsed_ms))
}

pub fn time() -> OsResult<Duration> {
    let mut ecode: u64;
    let mut elapsed_ms: u64;

    unsafe {
        asm!("svc $2
              mov $0, x0
              mov $1, x7"
             : "=r"(elapsed_ms), "=r"(ecode)
             : "i"(NR_TIME)
             : "x0", "x7"
             : "volatile");
    }

    err_or!(ecode, Duration::from_millis(elapsed_ms))
}

pub fn exit() -> ! {
    unsafe {
        asm!("svc $0"
             : // no output reg
             : "i"(NR_EXIT)
             :
             : "volatile");
    }
    loop {}
}

pub fn write(b: u8) {
    unsafe {
        asm!("mov x0, $0
              svc $1"
             : // no output reg
             : "r"(b), "i"(NR_WRITE)
             : "x0"
             : "volatile");
    }
}

pub fn getpid() -> OsResult<u64> {
    let mut ecode: u64;
    let mut pid: u64;

    unsafe {
        asm!("svc $2
              mov $0, x0
              mov $1, x7"
             : "=r"(pid), "=r"(ecode)
             : "i"(NR_GETPID)
             : "x0", "x7"
             : "volatile");
    }

    Ok(pid)
    // err_or!(ecode, pid)
}


struct Console;

impl fmt::Write for Console {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for b in s.bytes() {
            write(b);
        }
        Ok(())
    }
}

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => ($crate::syscall::vprint(format_args!($($arg)*)));
}

#[macro_export]
macro_rules! println {
 () => (print!("\n"));
    ($($arg:tt)*) => ({
        $crate::syscall::vprint(format_args!($($arg)*));
        $crate::print!("\n");
    })
}

pub fn vprint(args: fmt::Arguments) {
    let mut c = Console;
    c.write_fmt(args).unwrap();
}

use alloc::boxed::Box;
use core::time::Duration;

use crate::console::{CONSOLE, kprintln};
use crate::process::State;
use crate::traps::TrapFrame;
use crate::SCHEDULER;
use kernel_api::*;

/// Sleep for `ms` milliseconds.
///
/// This system call takes one parameter: the number of milliseconds to sleep.
///
/// In addition to the usual status value, this system call returns one
/// parameter: the approximate true elapsed time from when `sleep` was called to
/// when `sleep` returned.
pub fn sys_sleep(ms: u32, tf: &mut TrapFrame) {
    let start = pi::timer::current_time();
    let end = start + Duration::from_millis(ms.into());
    kprintln!("[syscall] start={:?} end={:?}", start, end);

    let state = State::Waiting(Box::new(move |p| {
        let now = pi::timer::current_time();
        if now >= end {
            kprintln!("returning! @ {:?}", now);
            p.context.xregs[0] = (now - start).as_millis() as u64;
            p.context.xregs[7] = OsError::Ok as u64;
            true
        } else {
            false
        }
    }));
    SCHEDULER.switch(state, tf);
}

/// Returns current time.
///
/// This system call does not take parameter.
///
/// In addition to the usual status value, this system call returns two
/// parameter:
///  - current time as seconds
///  - fractional part of the current time, in nanoseconds.
pub fn sys_time(tf: &mut TrapFrame) {
    let now = pi::timer::current_time();

    tf.xregs[0] = now.as_secs();
    tf.xregs[1] = now.subsec_nanos() as u64;
    tf.xregs[7] = OsError::Ok as u64;
    // unimplemented!("sys_time()");
}

/// Kills current process.
///
/// This system call does not take paramer and does not return any value.
pub fn sys_exit(tf: &mut TrapFrame) {
    if SCHEDULER.kill(tf).is_none() {
        kprintln!("Could not find process with ID: {}", tf.tpidr);
    }
}

/// Write to console.
///
/// This system call takes one parameter: a u8 character to print.
///
/// It only returns the usual status value.
pub fn sys_write(b: u8, tf: &mut TrapFrame) {
    let mut console = CONSOLE.lock();
    if b == b'\n' {
        console.write_byte(b'\r');
    }
    console.write_byte(b);
    tf.xregs[7] = OsError::Ok as u64;
    // unimplemented!("sys_write()");
}

/// Returns current process's ID.
///
/// This system call does not take parameter.
///
/// In addition to the usual status value, this system call returns a
/// parameter: the current process's ID.
pub fn sys_getpid(tf: &mut TrapFrame) {
    tf.xregs[0] = tf.tpidr;
    tf.xregs[7] = OsError::Ok as u64;
}

pub fn handle_syscall(num: u16, tf: &mut TrapFrame) {
    use crate::console::kprintln;
    match num as usize {
        NR_SLEEP => sys_sleep(tf.xregs[0] as u32, tf),
        NR_EXIT => sys_exit(tf),
        NR_TIME => sys_time(tf),
        NR_GETPID => sys_getpid(tf),
        NR_WRITE => sys_write(tf.xregs[0] as u8, tf),
        _ => unimplemented!("syscall not yet implemented"),
    }
}

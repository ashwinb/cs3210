use alloc::boxed::Box;
use alloc::collections::vec_deque::VecDeque;
use alloc::vec::Vec;
use core::fmt;

use aarch64::*;

use crate::mutex::Mutex;
use crate::param::{PAGE_MASK, PAGE_SIZE, TICK, USER_IMG_BASE, KERN_STACK_BASE};
use crate::process::{Id, Process, State};
use crate::traps::TrapFrame;
use crate::{VMM, IRQ};
use crate::shell;
use crate::console::kprintln;

use pi::interrupt as intr;
use kernel_api::syscall;

/// Process scheduler for the entire machine.
#[derive(Debug)]
pub struct GlobalScheduler(Mutex<Option<Scheduler>>);

#[no_mangle]
extern "C" fn roflcopter() {
    // unsafe { asm!("brk 1" :::: "volatile"); }
    // unsafe { asm!("brk 2" :::: "volatile"); }
    shell::shell("user0> ");
    // unsafe { asm!("brk 3" :::: "volatile"); }
    // loop { shell::shell("user1> "); }
}

#[no_mangle]
extern "C" fn print1() {
    loop {
        kprintln!("one");
        pi::timer::spin_sleep(core::time::Duration::from_millis(500));
    }
}

#[no_mangle]
extern "C" fn print2() {
    loop {
        kprintln!("I am two");
        let _ = syscall::sleep(core::time::Duration::from_millis(2000));
    }
}

impl GlobalScheduler {
    /// Returns an uninitialized wrapper around a local scheduler.
    pub const fn uninitialized() -> GlobalScheduler {
        GlobalScheduler(Mutex::new(None))
    }

    /// Enter a critical region and execute the provided closure with the
    /// internal scheduler.
    pub fn critical<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&mut Scheduler) -> R,
    {
        let mut guard = self.0.lock();
        f(guard.as_mut().expect("scheduler uninitialized"))
    }

    /// Adds a process to the scheduler's queue and returns that process's ID.
    /// For more details, see the documentation on `Scheduler::add()`.
    pub fn add(&self, process: Process) -> Option<Id> {
        self.critical(|scheduler| scheduler.add(process))
    }

    /// Performs a context switch using `tf` by setting the state of the current
    /// process to `new_state`, saving `tf` into the current process, and
    /// restoring the next process's trap frame into `tf`. For more details, see
    /// the documentation on `Scheduler::schedule_out()` and `Scheduler::switch_to()`.
    pub fn switch(&self, new_state: State, tf: &mut TrapFrame) -> Id {
        // kprintln!("==== now = {:?} ====", pi::timer::current_time());
        // self.critical(|scheduler| {
        //     for (i, proc) in scheduler.processes.iter().enumerate() {
        //         kprintln!("p{}: {:?}", i, proc);
        //     }
        // });
        self.critical(|scheduler| scheduler.schedule_out(new_state, tf));
        self.switch_to(tf)
    }

    pub fn switch_to(&self, tf: &mut TrapFrame) -> Id {
        loop {
            let rtn = self.critical(|scheduler| scheduler.switch_to(tf));
            if let Some(id) = rtn {
                return id;
            }
            aarch64::wfi();
        }
    }

    /// Kills currently running process and returns that process's ID.
    /// For more details, see the documentaion on `Scheduler::kill()`.
    #[must_use]
    pub fn kill(&self, tf: &mut TrapFrame) -> Option<Id> {
        self.critical(|scheduler| scheduler.kill(tf))
    }

    /// Starts executing processes in user space using timer interrupt based
    /// preemptive scheduling. This method should not return under normal conditions.
    pub fn start(&self) -> ! {
        let mut controller = intr::Controller::new();
        controller.enable(intr::Interrupt::Timer1);

        pi::timer::tick_in(TICK);
        IRQ.register(intr::Interrupt::Timer1, Box::new(|tf: &mut TrapFrame| {
            pi::timer::tick_in(TICK);
            crate::SCHEDULER.switch(State::Ready, tf);
        }));

        // we need to bootstrap the first process; at the very least we need to
        // "eret" to EL0 with _some_ reasonable PC
        let tf_ptr = self.critical(|s| {
            let tf_box_ref: &Box<_> = &s.processes.front().unwrap().context;
            &*(*tf_box_ref) as *const TrapFrame as usize
        });
        unsafe {
            asm!("mov sp, $0" :: "r"(tf_ptr) :: "volatile");
            asm!("bl context_restore");
            asm!("ldp x28, x29, [SP], #16");
            asm!("ldp lr, xzr, [SP], #16");
            // "N" prefix so we force using a 64-bit register for moving
            asm!("mov sp, $0" ::"N"(KERN_STACK_BASE):"x0":"volatile");
            asm!("eret");
        }
        loop {
            aarch64::nop();
        }
    }

    /// Initializes the scheduler and add userspace processes to the Scheduler
    pub unsafe fn initialize(&self) {
        let mut scheduler = Scheduler::new();

        // {
        //     let mut p = Process::new().unwrap();
        //     p.context.elr = roflcopter as u64;
        //     scheduler.add(p).unwrap();
        // }
        // {
        //     let mut p = Process::new().unwrap();
        //     self.test_phase_3(&mut p);

        //     kprintln!("allocating a process now");
        //     kprintln!("Kernel page table: {:#?}", VMM.debug_table());
        //     kprintln!("User page table: {:#?}", p.vmap);

        //     scheduler.add(p).unwrap();
        // }
        {
            let mut p = Process::load("/bin/sleep.bin").unwrap();
            scheduler.add(p).unwrap();
        }
        {
            let mut p = Process::load("/bin/fib.bin").unwrap();
            // kprintln!("Kernel page table: {:#?}", VMM.debug_table());
            // kprintln!("User page table: {:#?}", p.vmap);
            scheduler.add(p).unwrap();
        }
        {
            let mut p = Process::load("/bin/sleep.bin").unwrap();
            scheduler.add(p).unwrap();
        }
        {
            let mut p = Process::load("/bin/sleep.bin").unwrap();
            scheduler.add(p).unwrap();
        }
        // {
        //     let mut p = Process::load("/bin/fib.bin").unwrap();
        //     kprintln!("allocating another process now");
        //     scheduler.add(p).unwrap();
        // }

        *self.0.lock() = Some(scheduler);
    }

    // The following method may be useful for testing Phase 3:
    //
    // * A method to load a extern function to the user process's page table.
    //
    pub fn test_phase_3(&self, proc: &mut Process){
        use crate::vm::{PagePerm};

        let page = proc.vmap.alloc(USER_IMG_BASE.into(), PagePerm::RWX);
        let text = unsafe {
            core::slice::from_raw_parts(test_user_process as *const u8, 24)
        };

        page[0..24].copy_from_slice(text);
    }
}

#[derive(Debug)]
pub struct Scheduler {
    processes: VecDeque<Process>,
    last_id: Option<Id>,
}

impl Scheduler {
    /// Returns a new `Scheduler` with an empty queue.
    fn new() -> Scheduler {
        Scheduler {
            processes: VecDeque::new(),
            last_id: None,
        }
    }

    /// Adds a process to the scheduler's queue and returns that process's ID if
    /// a new process can be scheduled. The process ID is newly allocated for
    /// the process and saved in its `trap_frame`. If no further processes can
    /// be scheduled, returns `None`.
    ///
    /// It is the caller's responsibility to ensure that the first time `switch`
    /// is called, that process is executing on the CPU.
    fn add(&mut self, mut process: Process) -> Option<Id> {
        self.last_id = match self.last_id {
            None => Some(1),
            Some(lid) => lid.checked_add(1)
        };
        process.context.tpidr = self.last_id?;
        self.processes.push_back(process);

        self.last_id
    }

    /// Finds the currently running process, sets the current process's state
    /// to `new_state`, prepares the context switch on `tf` by saving `tf`
    /// into the current process, and push the current process back to the
    /// end of `processes` queue.
    ///
    /// If the `processes` queue is empty or there is no current process,
    /// returns `false`. Otherwise, returns `true`.
    fn schedule_out(&mut self, new_state: State, tf: &mut TrapFrame) -> bool {
        // kprintln!("trying to schedule out PID ({}) with state {:?}", tf.tpidr, new_state);
        if let Some(pos) = self.processes.iter().position(|p| p.context.tpidr == tf.tpidr) {
            // what if the status of this process is not running?
            let mut proc = self.processes.remove(pos).unwrap();
            proc.state = new_state;
            *proc.context = *tf;
            self.processes.push_back(proc);
            true
        } else {
            false
        }
    }

    /// Finds the next process to switch to, brings the next process to the
    /// front of the `processes` queue, changes the next process's state to
    /// `Running`, and performs context switch by restoring the next process`s
    /// trap frame into `tf`.
    ///
    /// If there is no process to switch to, returns `None`. Otherwise, returns
    /// `Some` of the next process`s process ID.
    fn switch_to(&mut self, tf: &mut TrapFrame) -> Option<Id> {
        // kprintln!("Finding a process to switch to...");
        let pos = self.processes.iter_mut().position(|p| p.is_ready())?;
        let mut proc = self.processes.remove(pos).unwrap();
        proc.state = State::Running;
        *tf = *proc.context;
        self.processes.push_front(proc);
        Some(tf.tpidr)
    }

    /// Kills currently running process by scheduling out the current process
    /// as `Dead` state. Removes the dead process from the queue, drop the
    /// dead process's instance, and returns the dead process's process ID.
    fn kill(&mut self, tf: &mut TrapFrame) -> Option<Id> {
        kprintln!("[scheduler] attempting to kill process {} #processes = {}", tf.tpidr, self.processes.len());
        if self.schedule_out(State::Dead, tf) {
            self.processes.pop_back()?;
            Some(tf.tpidr)
        } else {
            None
        }
    }
}

pub extern "C" fn test_user_process() -> ! {
    loop {
        let ms = 10000;
        let error: u64;
        let elapsed_ms: u64;

        unsafe {
            asm!("mov x0, $2
              svc 1
              mov $0, x0
              mov $1, x7"
                 : "=r"(elapsed_ms), "=r"(error)
                 : "r"(ms)
                 : "x0", "x7"
                 : "volatile");
        }
    }
}

use alloc::boxed::Box;
use shim::io;
use shim::path::Path;
use core::fmt;
use aarch64::regs::SPSR_EL1;

use crate::param::*;
use crate::process::{Stack, State};
use crate::traps::TrapFrame;
use crate::vm::*;
use kernel_api::{OsError, OsResult};
use crate::{VMM, FILESYSTEM, kprintln};
use fat32::traits::{File, Entry, FileSystem};
use io::Read;

/// Type alias for the type of a process ID.
pub type Id = u64;

/// A structure that represents the complete state of a process.
pub struct Process {
    /// The saved trap frame of a process.
    pub context: Box<TrapFrame>,
    /// The memory allocation used for the process's stack.
    // pub stack: Stack,
    /// The page table describing the Virtual Memory of the process
    pub vmap: UserPageTable,
    /// The scheduling state of the process.
    pub state: State,
}

impl fmt::Debug for Process {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Process")
            .field("id", &self.context.tpidr)
            .field("tf", &self.context)
            .field("state", &self.state)
            .finish()
    }
}

impl Process {
    /// Creates a new process with a zeroed `TrapFrame` (the default), a zeroed
    /// stack of the default size, and a state of `Ready`.
    ///
    /// If enough memory could not be allocated to start the process, returns
    /// `None`. Otherwise returns `Some` of the new `Process`.
    pub fn new() -> OsResult<Process> {
        let vmap = UserPageTable::new();
        let mut context = Box::new(TrapFrame::default());
        context.sp = Self::get_stack_top().as_u64();
        kprintln!("stack pointer {:0x}", context.sp);
        context.elr = Self::get_image_base().as_u64();
        context.ttbr[0] = VMM.get_baddr().as_u64();
        context.ttbr[1] = vmap.get_baddr().as_u64();
        context.spsr = SPSR_EL1::F | SPSR_EL1::A | SPSR_EL1::D;

        Ok(Process {
            context,
            vmap,
            state: State::Ready,
        })
    }

    /// Load a program stored in the given path by calling `do_load()` method.
    /// Set trapframe `context` corresponding to the its page table.
    /// `sp` - the address of stack top
    /// `elr` - the address of image base.
    /// `ttbr0` - the base address of kernel page table
    /// `ttbr1` - the base address of user page table
    /// `spsr` - `F`, `A`, `D` bit should be set.
    ///
    /// Returns Os Error if do_load fails.
    /// Creates a process and open a file with given path.
    /// Allocates one page for stack with read/write permission, and N pages with read/write/execute
    /// permission to load file's contents.
    pub fn load<P: AsRef<Path>>(path: P) -> OsResult<Process> {
        let mut file = FILESYSTEM.open(&path)?.into_file().ok_or(OsError::IoErrorInvalidInput)?;

        let mut addr = USER_IMG_BASE;
        let mut proc = Process::new()?;
        let mut page = proc.vmap.alloc(addr.into(), PagePerm::RWX);
        let mut total = 0;
        loop {
            match file.read(&mut page) {
                Err(ref e) if e.kind() == io::ErrorKind::Interrupted => continue,
                Err(e) => return Err(e.into()),
                Ok(n) => {
                    total += n;
                    if n == 0 {
                        // there's a boundary condition if the file ends at a page boundary
                        // we will end up allocating a page but not using it, we should de-allocate
                        // in that case
                        break;
                    }
                    if n == page.len() {
                        addr += PAGE_SIZE;
                        page = proc.vmap.alloc(addr.into(), PagePerm::RWX);
                    } else {
                        page = &mut page[n..];
                    }
                }
            }
        }
        kprintln!("read {} bytes for file {:?}", total, path.as_ref());
        // allocate stack
        for addr in (Self::get_stack_base().as_usize() .. Self::get_stack_top().as_usize()).step_by(PAGE_SIZE) {
            let _ = proc.vmap.alloc(addr.into(), PagePerm::RW);
        }

        Ok(proc)
        // unimplemented!();
    }

    /// Returns the highest `VirtualAddr` that is supported by this system.
    pub fn get_max_va() -> VirtualAddr {
        unimplemented!();
    }

    /// Returns the `VirtualAddr` represents the base address of the user
    /// memory space.
    pub fn get_image_base() -> VirtualAddr {
        USER_IMG_BASE.into()
        // unimplemented!();
    }

    /// Returns the `VirtualAddr` represents the base address of the user
    /// process's stack.
    pub fn get_stack_base() -> VirtualAddr {
        let base = Self::get_stack_top().as_usize() - Stack::SIZE;
        (base & PAGE_MASK).into()
        // unimplemented!();
    }

    /// Returns the `VirtualAddr` represents the top of the user process's
    /// stack.
    pub fn get_stack_top() -> VirtualAddr {
        let aligned_top = core::usize::MAX & !(PAGE_ALIGN - 1);
        aligned_top.into()
        // unimplemented!();
    }

    /// Returns `true` if this process is ready to be scheduled.
    ///
    /// This functions returns `true` only if one of the following holds:
    ///
    ///   * The state is currently `Ready`.
    ///
    ///   * An event being waited for has arrived.
    ///
    ///     If the process is currently waiting, the corresponding event
    ///     function is polled to determine if the event being waiting for has
    ///     occured. If it has, the state is switched to `Ready` and this
    ///     function returns `true`.
    ///
    /// Returns `false` in all other cases.
    pub fn is_ready(&mut self) -> bool {
        // temporarily replace so we can avoid borrowing self multiple times :((
        let mut state = core::mem::replace(&mut self.state, State::Ready);

        let is_ready = match state {
            State::Ready => true,
            State::Waiting(ref mut f) => f(self),
            _ => false,
        };

        if !is_ready {
            core::mem::replace(&mut self.state, state);
        }
        is_ready
    }
}

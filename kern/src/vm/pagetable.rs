use core::iter::Chain;
use core::ops::{Deref, DerefMut};
use core::slice::Iter;

use alloc::boxed::Box;
use alloc::fmt;
use core::alloc::{GlobalAlloc, Layout};

use crate::allocator;
use crate::param::*;
use crate::vm::{PhysicalAddr, VirtualAddr};
use crate::ALLOCATOR;

use aarch64::vmsa::*;
use shim::const_assert_size;
use crate::{kprintln};

#[repr(C)]
pub struct Page([u8; PAGE_SIZE]);
const_assert_size!(Page, PAGE_SIZE);

impl Page {
    pub const SIZE: usize = PAGE_SIZE;
    pub const ALIGN: usize = PAGE_SIZE;

    fn layout() -> Layout {
        unsafe { Layout::from_size_align_unchecked(Self::SIZE, Self::ALIGN) }
    }
}

#[repr(C)]
#[repr(align(65536))]
pub struct L2PageTable {
    pub entries: [RawL2Entry; 8192],
}
const_assert_size!(L2PageTable, PAGE_SIZE);

impl L2PageTable {
    /// Returns a new `L2PageTable`
    fn new() -> L2PageTable {
        L2PageTable { entries: [RawL2Entry::new(0); 8192] }
    }

    /// Returns a `PhysicalAddr` of the pagetable.
    pub fn as_ptr(&self) -> PhysicalAddr {
        assert!(self as *const L2PageTable as usize == self.entries.as_ptr() as usize);
        (self as *const L2PageTable).into()
    }
}

#[derive(Copy, Clone)]
pub struct L3Entry(RawL3Entry);

impl L3Entry {
    /// Returns a new `L3Entry`.
    fn new() -> L3Entry {
        L3Entry(RawL3Entry::new(0))
    }

    /// Returns `true` if the L3Entry is valid and `false` otherwise.
    fn is_valid(&self) -> bool {
        self.0.get_masked(RawL3Entry::VALID) != 0
    }

    /// Extracts `ADDR` field of the L3Entry and returns as a `PhysicalAddr`
    /// if valid. Otherwise, return `None`.
    fn get_page_addr(&self) -> Option<PhysicalAddr> {
        if self.is_valid() {
            Some(self.0.get_masked(RawL3Entry::ADDR).into())
        } else {
            None
        }
    }
}

impl fmt::Debug for L3Entry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

#[repr(C)]
#[repr(align(65536))]
pub struct L3PageTable {
    pub entries: [L3Entry; 8192],
}
const_assert_size!(L3PageTable, PAGE_SIZE);

impl L3PageTable {
    /// Returns a new `L3PageTable`.
    fn new() -> L3PageTable {
        L3PageTable { entries: [L3Entry::new(); 8192] }
    }

    /// Returns a `PhysicalAddr` of the pagetable.
    pub fn as_ptr(&self) -> PhysicalAddr {
        assert!(self as *const L3PageTable as usize == self.entries.as_ptr() as usize);
        (self as *const L3PageTable).into()
    }
}

#[repr(C)]
#[repr(align(65536))]
pub struct PageTable {
    pub l2: L2PageTable,
    pub l3: [L3PageTable; 2],
}

impl PageTable {
    /// Returns a new `Box` containing `PageTable`.
    /// Entries in L2PageTable should be initialized properly before return.
    fn new(perm: u64) -> Box<PageTable> {
        let mut table = Box::new(PageTable {
            l2: L2PageTable::new(),
            l3: [L3PageTable::new(), L3PageTable::new()],
        });
        kprintln!("l2 addr: {:0x}, l3 addresses: {:0x} {:0x}",
            table.l2.as_ptr().as_usize(),
            table.l3[0].as_ptr().as_usize(),
            table.l3[1].as_ptr().as_usize());

        for (i, l3table) in table.l3.iter().enumerate() {
            table.l2.entries[i]
                .set_bit(RawL2Entry::AF)
                .set_value(EntryType::Table, RawL2Entry::TYPE)
                .set_value(EntryValid::Valid, RawL2Entry::VALID)
                .set_value(perm, RawL2Entry::AP)
                .set_masked(l3table.as_ptr().as_u64(), RawL2Entry::ADDR);
        }
        table
    }

    /// Returns the (L2index, L3index) extracted from the given virtual address.
    /// Since we are only supporting 1GB virtual memory in this system, L2index
    /// should be smaller than 2.
    ///
    /// # Panics
    ///
    /// Panics if the virtual address is not properly aligned to page size.
    /// Panics if extracted L2index exceeds the number of L3PageTable.
    fn locate(va: VirtualAddr) -> (usize, usize) {
        let mut va = va.as_usize();
        assert!((va & !PAGE_MASK) == 0, "badly aligned addr? {:x} {:0x} {:0x}", va, !PAGE_MASK, va & !PAGE_MASK);
        if va >= USER_IMG_BASE {
            va = va & USER_IMG_MASK;
        }

        let va = VirtualAddrBits::new(va as u64);
        let (l2idx, l3idx) = (va.get_value(VirtualAddrBits::L2), va.get_value(VirtualAddrBits::L3));
        assert!(l2idx < 2, "L2index value too large {}", l2idx);
        (l2idx as usize, l3idx as usize)
        // unimplemented!("PageTable::localte()")
    }

    pub fn entry(&mut self, va: VirtualAddr) -> &mut L3Entry {
        let (l2idx, l3idx) = PageTable::locate(va);
        &mut self.l3[l2idx].entries[l3idx]
    }

    /// Returns `true` if the L3entry indicated by the given virtual address is valid.
    /// Otherwise, `false` is returned.
    // pub fn is_valid(&self, va: VirtualAddr) -> bool {
    //     self.entry(va).is_valid()
        // let (l2idx, l3idx) = PageTable::locate(va);
        // self.l3[l2idx].entries[l3idx].is_valid()
        // unimplemented!("PageTable::is_valid()")
    // }

    /// Set the given RawL3Entry `entry` to the L3Entry indicated by the given virtual
    /// address.
    pub fn set_entry(&mut self, va: VirtualAddr, entry: RawL3Entry) -> &mut Self {
        let (l2idx, l3idx) = PageTable::locate(va);
        self.l3[l2idx].entries[l3idx].0 = entry;
        self
        // unimplemented!("PageTable::set_entry()")
    }

    /// Returns a base address of the pagetable. The returned `PhysicalAddr` value
    /// will point the start address of the L2PageTable.
    pub fn get_baddr(&self) -> PhysicalAddr {
        assert!(self as *const PageTable as usize == self.l2.as_ptr().as_usize());
        (self as *const PageTable).into()
    }
}

impl fmt::Debug for PageTable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("L2: ")?;
        f.debug_list().entries(self.l2.entries[0..5].iter()).finish()?;
        if f.alternate() {
            f.write_str("\n")?;
        }
        f.write_str("First L3: ")?;
        f.debug_list().entries(self.l3[0].entries[0..32].iter()).finish()?;
        if f.alternate() {
            f.write_str("\n")?;
        }
        f.write_str("Second L3: ")?;
        f.debug_list().entries(self.l3[1].entries[0..16].iter()).finish()?;
        f.debug_list().entries(self.l3[1].entries.iter().rev().take(8)).finish()?;
        if f.alternate() {
            f.write_str("\n")?;
            f.write_str("================\n")?;
        }
        Ok(())
    }
}


// FIXME: Implement `IntoIterator` for `&PageTable`.

#[derive(Debug)]
pub struct KernPageTable(Box<PageTable>);

impl KernPageTable {
    /// Returns a new `KernPageTable`. `KernPageTable` should have a `Pagetable`
    /// created with `KERN_RW` permission.
    ///
    /// Set L3entry of ARM physical address starting at 0x00000000 for RAM and
    /// physical address range from `IO_BASE` to `IO_BASE_END` for peripherals.
    /// Each L3 entry should have correct value for lower attributes[10:0] as well
    /// as address[47:16]. Refer to the definition of `RawL3Entry` in `vmsa.rs` for
    /// more details.
    pub fn new() -> KernPageTable {
        let perm = EntryPerm::KERN_RW;
        let mut table = PageTable::new(perm);

        let (_, mut end) = allocator::memory_map().expect("failed to find memory map");
        end = allocator::util::align_down(end, PAGE_SIZE);

        for addr in (0 .. end).step_by(PAGE_SIZE) {
            let mut entry = RawL3Entry::new(0);
            entry
                .set_bit(RawL3Entry::AF)
                .set_value(PageType::Page, RawL3Entry::TYPE)
                .set_value(EntryValid::Valid, RawL3Entry::VALID)
                .set_value(perm, RawL3Entry::AP)
                .set_value(EntryAttr::Normal, RawL3Entry::ATTR)
                .set_value(EntrySh::Inner, RawL3Entry::SH)
                .set_masked(addr as u64, RawL3Entry::ADDR);
            table.set_entry(addr.into(), entry);
        }

        for addr in (IO_BASE .. IO_BASE_END).step_by(PAGE_SIZE) {
            let mut entry = RawL3Entry::new(0);
            entry
                .set_bit(RawL3Entry::AF)
                .set_value(PageType::Page, RawL3Entry::TYPE)
                .set_value(EntryValid::Valid, RawL3Entry::VALID)
                .set_value(perm, RawL3Entry::AP)
                .set_value(EntryAttr::Device, RawL3Entry::ATTR)
                .set_value(EntrySh::Outer, RawL3Entry::SH)
                .set_masked(addr as u64, RawL3Entry::ADDR);
            table.set_entry(addr.into(), entry);
        }
        KernPageTable(table)
        // unimplemented!("KernPageTable::new()")
    }
}

pub enum PagePerm {
    RW,
    RO,
    RWX,
}

#[derive(Debug)]
pub struct UserPageTable(Box<PageTable>);

impl UserPageTable {
    /// Returns a new `UserPageTable` containing a `PageTable` created with
    /// `USER_RW` permission.
    pub fn new() -> UserPageTable {
        UserPageTable(PageTable::new(EntryPerm::USER_RW))
        // unimplemented!("UserPageTable::new()")
    }

    /// Allocates a page and set an L3 entry translates given virtual address to the
    /// physical address of the allocated page. Returns the allocated page.
    ///
    /// # Panics
    /// Panics if the virtual address is lower than `USER_IMG_BASE`.
    /// Panics if the virtual address has already been allocated.
    /// Panics if allocator fails to allocate a page.
    ///
    /// TODO. use Result<T> and make it failurable
    /// TODO. use perm properly
    pub fn alloc(&mut self, va: VirtualAddr, _perm: PagePerm) -> &mut [u8] {
        assert!(va.as_usize() >= USER_IMG_BASE, "addr from invalid range: {:0x}", va.as_usize());

        let l3entry = self.entry(va);
        assert!(!l3entry.is_valid(), "page already allocated?");

        let ptr = unsafe { ALLOCATOR.alloc(Page::layout()) };
        assert!(!ptr.is_null(), "failed allocation");

        let mut raw_entry = RawL3Entry::new(0);
        raw_entry
            .set_bit(RawL3Entry::AF)
            .set_value(PageType::Page, RawL3Entry::TYPE)
            .set_value(EntryValid::Valid, RawL3Entry::VALID)
            .set_value(EntryPerm::USER_RW, RawL3Entry::AP)
            .set_value(EntryAttr::Normal, RawL3Entry::ATTR)
            .set_value(EntrySh::Inner, RawL3Entry::SH)
            .set_masked(ptr as u64, RawL3Entry::ADDR);
        l3entry.0 = raw_entry;

        let page = unsafe { &mut *(ptr as *mut Page) };
        page.0.as_mut()
        // unimplemented!("alloc()");
    }
}

impl Deref for KernPageTable {
    type Target = PageTable;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Deref for UserPageTable {
    type Target = PageTable;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for KernPageTable {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl DerefMut for UserPageTable {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

// FIXME: Implement `Drop` for `UserPageTable`.
impl Drop for UserPageTable {
    fn drop(&mut self) {
        for table in self.l3.iter() {
            for entry in table.entries.iter() {
                if let Some(addr) = entry.get_page_addr() {
                    unsafe {
                        ALLOCATOR.dealloc(addr.as_ptr() as *mut u8, Page::layout());
                    }
                }
            }
        }
    }
}

// FIXME: Implement `fmt::Debug` as you need.


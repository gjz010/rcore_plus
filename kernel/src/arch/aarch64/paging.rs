//! Page table implementations for aarch64.
use aarch64::asm::{tlb_invalidate, tlb_invalidate_all, ttbr_el1_read, ttbr_el1_write};
use aarch64::paging::memory_attribute::*;
use aarch64::paging::{FrameAllocator, FrameDeallocator, Page, PhysFrame as Frame, Size4KiB};
use aarch64::paging::{
    Mapper, PageTable as Aarch64PageTable, PageTableEntry, PageTableFlags as EF, RecursivePageTable,
};
use aarch64::{PhysAddr, VirtAddr};
use log::*;
use rcore_memory::paging::*;
// Depends on kernel
use crate::consts::{KERNEL_OFFSET, KERNEL_PML4, RECURSIVE_INDEX};
use crate::memory::{active_table, alloc_frame, dealloc_frame};

pub struct ActivePageTable(RecursivePageTable<'static>);

pub struct PageEntry(PageTableEntry);

impl PageTable for ActivePageTable {
    fn map(&mut self, addr: usize, target: usize) -> &mut Entry {
        let val=&self.0.p4.entries[RECURSIVE_INDEX].addr();
        info!("PageTable(at 0x{:x})::map Mapping 0x{:x} to 0x{:x}", val, target, addr);
        let flags = EF::default();
        let attr = MairNormal::attr_value();
        self.0
            .map_to(
                Page::of_addr(addr as u64),
                Frame::of_addr(target as u64),
                flags,
                attr,
                &mut FrameAllocatorForAarch64,
            )
            .unwrap()
            .flush();
        info!("Done.");
        self.get_entry(addr).expect("fail to get entry")

    }

    fn unmap(&mut self, addr: usize) {
        let (_frame, flush) = self.0.unmap(Page::of_addr(addr as u64)).unwrap();
        flush.flush();
    }

    fn get_entry(&mut self, vaddr: usize) -> Option<&mut Entry> {
        // get p1 entry
        let entry_addr =
            ((vaddr >> 9) & 0o777_777_777_7770) | (RECURSIVE_INDEX << 39) | (vaddr & KERNEL_OFFSET);
        Some(unsafe { &mut *(entry_addr as *mut PageEntry) })
    }
}

impl PageTableExt for ActivePageTable {
    const TEMP_PAGE_ADDR: usize = KERNEL_OFFSET | 0xcafeb000;
}

const ROOT_PAGE_TABLE: *mut Aarch64PageTable = (KERNEL_OFFSET
    | (RECURSIVE_INDEX << 39)
    | (RECURSIVE_INDEX << 30)
    | (RECURSIVE_INDEX << 21)
    | (RECURSIVE_INDEX << 12))
    as *mut Aarch64PageTable;

impl ActivePageTable {
    pub unsafe fn new() -> Self {
        ActivePageTable(RecursivePageTable::new(&mut *(ROOT_PAGE_TABLE as *mut _)).unwrap())
    }
}

#[repr(u8)]
pub enum MMIOType {
    Normal = 0,
    Device = 1,
    NormalNonCacheable = 2,
    Unsupported = 3,
}

impl Entry for PageEntry {
    fn update(&mut self) {
        let addr = VirtAddr::new_unchecked((self as *const _ as u64) << 9);
        tlb_invalidate(addr);
    }

    fn present(&self) -> bool {
        self.0.flags().contains(EF::VALID)
    }
    fn accessed(&self) -> bool {
        self.0.flags().contains(EF::AF)
    }
    fn writable(&self) -> bool {
        self.0.flags().contains(EF::WRITE)
    }
    fn dirty(&self) -> bool {
        self.hw_dirty() && self.sw_dirty()
    }

    fn clear_accessed(&mut self) {
        self.as_flags().remove(EF::AF);
    }
    fn clear_dirty(&mut self) {
        self.as_flags().remove(EF::DIRTY);
        self.as_flags().insert(EF::AP_RO);
    }
    fn set_writable(&mut self, value: bool) {
        self.as_flags().set(EF::AP_RO, !value);
        self.as_flags().set(EF::WRITE, value);
    }
    fn set_present(&mut self, value: bool) {
        self.as_flags().set(EF::VALID, value);
    }
    fn target(&self) -> usize {
        self.0.addr().as_u64() as usize
    }
    fn set_target(&mut self, target: usize) {
        self.0.modify_addr(PhysAddr::new(target as u64));
    }
    fn writable_shared(&self) -> bool {
        self.0.flags().contains(EF::WRITABLE_SHARED)
    }
    fn readonly_shared(&self) -> bool {
        self.0.flags().contains(EF::READONLY_SHARED)
    }
    fn set_shared(&mut self, writable: bool) {
        let flags = self.as_flags();
        flags.set(EF::WRITABLE_SHARED, writable);
        flags.set(EF::READONLY_SHARED, !writable);
    }
    fn clear_shared(&mut self) {
        self.as_flags()
            .remove(EF::WRITABLE_SHARED | EF::READONLY_SHARED);
    }
    fn user(&self) -> bool {
        self.0.flags().contains(EF::AP_EL0)
    }
    fn swapped(&self) -> bool {
        self.0.flags().contains(EF::SWAPPED)
    }
    fn set_swapped(&mut self, value: bool) {
        self.as_flags().set(EF::SWAPPED, value);
    }
    fn set_user(&mut self, value: bool) {
        self.as_flags().set(EF::AP_EL0, value);
        self.as_flags().set(EF::nG, value); // set non-global to use ASID
    }
    fn execute(&self) -> bool {
        if self.user() {
            !self.0.flags().contains(EF::UXN)
        } else {
            !self.0.flags().contains(EF::PXN)
        }
    }
    fn set_execute(&mut self, value: bool) {
        if self.user() {
            self.as_flags().set(EF::UXN, !value)
        } else {
            self.as_flags().set(EF::PXN, !value)
        }
    }
    fn mmio(&self) -> u8 {
        let value = self.0.attr().value;
        if value == MairNormal::attr_value().value {
            0
        } else if value == MairDevice::attr_value().value {
            1
        } else if value == MairNormalNonCacheable::attr_value().value {
            2
        } else {
            3
        }
    }
    fn set_mmio(&mut self, value: u8) {
        let attr = match value {
            0 => MairNormal::attr_value(),
            1 => MairDevice::attr_value(),
            2 => MairNormalNonCacheable::attr_value(),
            _ => return,
        };
        self.0.modify_attr(attr);
    }
}

impl PageEntry {
    fn read_only(&self) -> bool {
        self.0.flags().contains(EF::AP_RO)
    }
    fn hw_dirty(&self) -> bool {
        self.writable() && !self.read_only()
    }
    fn sw_dirty(&self) -> bool {
        self.0.flags().contains(EF::DIRTY)
    }
    fn as_flags(&mut self) -> &mut EF {
        unsafe { &mut *(self as *mut _ as *mut EF) }
    }
}

#[derive(Debug)]
pub struct InactivePageTable0 {
    p4_frame: Frame,
}
static mut last_bare_map:usize=0;
impl InactivePageTable for InactivePageTable0 {
    type Active = ActivePageTable;

    fn new() -> Self {
        // When the new InactivePageTable is created for the user MemorySet, it's use ttbr1 as the
        // TTBR. And the kernel TTBR ttbr0 will never changed, so we needn't call map_kernel()
        Self::new_bare()
    }

    fn new_bare() -> Self {

        let target = alloc_frame().expect("failed to allocate frame");
        info!("Creating a bare map at target 0x{:x}.", target);
        let frame = Frame::of_addr(target as u64);
        active_table().with_temporary_map(target, |_, table: &mut Aarch64PageTable| {
            table.zero();
            // set up recursive mapping for the table
            table[RECURSIVE_INDEX].set_frame(
                frame.clone(),
                EF::default(),
                MairNormal::attr_value(),
            );
            info!("Bare map now: {:?}", table);
        });
        unsafe{last_bare_map=target;}
        info!("Bare map created.");
        InactivePageTable0 { p4_frame: frame }
    }

    fn map_kernel(&mut self) {
        let table = unsafe { &mut *ROOT_PAGE_TABLE };
        let e0 = table[KERNEL_PML4].clone();
        assert!(!e0.is_unused());

        self.edit(|_| {
            table[KERNEL_PML4].set_frame(
                Frame::containing_address(e0.addr()),
                EF::default(),
                MairNormal::attr_value(),
            );
        });
    }

    fn token(&self) -> usize {
        self.p4_frame.start_address().as_u64() as usize // as TTBRx_EL1
    }

    unsafe fn set_token(token: usize) {
        ttbr_el1_write(0, Frame::containing_address(PhysAddr::new(token as u64)));
    }

    fn active_token() -> usize {
        ttbr_el1_read(0).start_address().as_u64() as usize
    }

    fn flush_tlb() {
        tlb_invalidate_all();
    }

    fn edit<T>(&mut self, f: impl FnOnce(&mut Self::Active) -> T) -> T {
        let target = ttbr_el1_read(1).start_address().as_u64() as usize;
        active_table().with_temporary_map(
            target,
            |active_table, p4_table: &mut Aarch64PageTable| {
                let target_table=self.p4_frame.start_address.0;
                info!("{},{}",target_table,unsafe{last_bare_map} as u64);
                let is_new=target_table==unsafe{last_bare_map} as u64;
                unsafe{last_bare_map=0;}
                if is_new{info!("S1 : {:?}", p4_table);}
                let backup = p4_table[RECURSIVE_INDEX].clone();
                let old_frame = ttbr_el1_read(0);
                if is_new{info!("S2 now: {:?}", p4_table);}
                // overwrite recursive mapping
                p4_table[RECURSIVE_INDEX].set_frame(
                    self.p4_frame.clone(),
                    EF::default(),
                    MairNormal::attr_value(),
                );
                if is_new{info!("S3 now: {:?}", p4_table);}
                ttbr_el1_write(0, self.p4_frame.clone());
                tlb_invalidate_all();
                if is_new{info!("S4 now: {:?}", p4_table);}
                // execute f in the new context
                let ret = f(active_table);

                // restore recursive mapping to original p4 table
                p4_table[RECURSIVE_INDEX] = backup;
                ttbr_el1_write(0, old_frame);
                tlb_invalidate_all();
                ret
            },
        )
    }
}

impl InactivePageTable0 {
    /// Activate as kernel page table (TTBR0).
    /// Used in `arch::memory::remap_the_kernel()`.
    pub unsafe fn activate_as_kernel(&self) {
        let old_frame = ttbr_el1_read(1);
        let new_frame = self.p4_frame.clone();
        debug!("switch TTBR1 {:?} -> {:?}", old_frame, new_frame);
        if old_frame != new_frame {
            ttbr_el1_write(0, Frame::of_addr(0));
            ttbr_el1_write(1, new_frame);
            tlb_invalidate_all();
        }
    }
}

impl Drop for InactivePageTable0 {
    fn drop(&mut self) {
        info!("PageTable dropping: {:?}", self);
        dealloc_frame(self.p4_frame.start_address().as_u64() as usize);
    }
}

struct FrameAllocatorForAarch64;

impl FrameAllocator<Size4KiB> for FrameAllocatorForAarch64 {
    fn alloc(&mut self) -> Option<Frame> {
        alloc_frame().map(|addr| Frame::of_addr(addr as u64))
    }
}

impl FrameDeallocator<Size4KiB> for FrameAllocatorForAarch64 {
    fn dealloc(&mut self, frame: Frame) {
        dealloc_frame(frame.start_address().as_u64() as usize);
    }
}

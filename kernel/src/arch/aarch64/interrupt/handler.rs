//! Trap handler

use super::context::TrapFrame;
use super::syndrome::{Fault, Syndrome};
use crate::arch::board::irq::handle_irq;

use aarch64::regs::*;
use log::*;
use rcore_memory::paging::{PageTable, Entry};
use core::fmt;
use core::fmt::Error;
use core::mem::ManuallyDrop;
use aarch64::VirtAddr;
use super::super::paging::PageTableImpl;
use aarch64::paging::mapper::{TranslateResult, mapped_page_table::PageTableWalkError};
use aarch64::paging::PhysFrame;
global_asm!(include_str!("trap.S"));
global_asm!(include_str!("vector.S"));

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
pub extern "C" fn rust_trap(info: Info, esr: u32, tf: &mut TrapFrame) {
    trace!("Interrupt: {:?}, ELR: {:#x?}", info, tf.elr);
    match info.kind {
        Kind::Synchronous => {
            let syndrome = Syndrome::from(esr);
            trace!("ESR: {:#x?}, Syndrome: {:?}", esr, syndrome);
            // syndrome is only valid with sync
            match syndrome {
                Syndrome::Brk(brk) => handle_break(brk, tf),
                Syndrome::Svc(svc) => handle_syscall(svc, tf),
                Syndrome::DataAbort { kind, level: _ }
                | Syndrome::InstructionAbort { kind, level: _ } => match kind {
                    Fault::Translation | Fault::AccessFlag | Fault::Permission => {
                        handle_page_fault(tf, esr, &syndrome)
                    }
                    _ => crate::trap::error(tf),
                },
                _ => crate::trap::error(tf),
            }
        }
        Kind::Irq => handle_irq(tf),
        _ => crate::trap::error(tf),
    }
    trace!("Interrupt end");
}

fn handle_break(_num: u16, tf: &mut TrapFrame) {
    // Skip the current brk instruction (ref: J1.1.2, page 6147)
    tf.elr += 4;
}

fn handle_syscall(num: u16, tf: &mut TrapFrame) {
    if num != 0 {
        crate::trap::error(tf);
    }

    // svc instruction has been skipped in syscall (ref: J1.1.2, page 6152)
    let ret = crate::syscall::syscall(
        tf.x1to29[7] as usize,
        [
            tf.x0,
            tf.x1to29[0],
            tf.x1to29[1],
            tf.x1to29[2],
            tf.x1to29[3],
            tf.x1to29[4],
        ],
        tf,
    );
    tf.x0 = ret as usize;
}

fn handle_page_fault(tf: &mut TrapFrame, esr: u32, syndrome: &Syndrome) {
    let addr = FAR_EL1.get() as usize;
    if addr&0xef000==0xef000{
        let mut pagetable= unsafe { super::super::paging::PageTableImpl::active() };
        unroll_pagetable(&VirtAddr::new(tf.elr as u64), &pagetable);
    }
    if !crate::memory::handle_page_fault(addr) {
        error!("\nEXCEPTION: Page Fault @ {:#x}", addr);
        error!("\nSyndrome: {:?} esr: {:#x}", syndrome, esr);
        let mut pagetable= unsafe { super::super::paging::PageTableImpl::active() };
        unroll_pagetable(&VirtAddr::new(tf.elr as u64), &pagetable);
        crate::trap::error(tf);
    }
}

fn print_entry(entry: &Entry){
        info!("impl Entry accessed={} dirty={} writable={} present={} target={} writable_shared={} readonly_shared={} swapped={} user={} execute={} mmio={}",
              entry.accessed(), entry.dirty(), entry.writable(), entry.present(), entry.target(), entry.writable_shared(), entry.readonly_shared(), entry.swapped(), entry.user(), entry.execute(), entry.mmio())
}

fn unroll_pagetable(addr: &VirtAddr, pagetable: &ManuallyDrop<PageTableImpl>){
    let pt=&pagetable.page_table;
    let p4 = &pt.level_4_table;
    info!("p4_entry found.");
    info!("{:?}", &p4[addr.p4_index()]);
    let p3 = match pt.page_table_walker.next_table(&p4[addr.p4_index()]) {
        Ok(page_table) => page_table,
        Err(PageTableWalkError::NotMapped) => {info!("PageNotMapped at P4"); return;},
        Err(PageTableWalkError::MappedToHugePage) => {
            panic!("level 4 entry has huge page bit set");
            return;
        }
    };
    info!("p3_entry found.");
    info!("{:?}", &p3[addr.p3_index()]);
    let p2 = match pt.page_table_walker.next_table(&p3[addr.p3_index()]) {
        Ok(page_table) => page_table,
        Err(PageTableWalkError::NotMapped) => {info!("PageNotMapped at P3"); return;},
        Err(PageTableWalkError::MappedToHugePage) => {
            info!("1GB frame detected.");
            return;
        }
    };
    info!("p2_entry found.");
    info!("{:?}", &p2[addr.p2_index()]);
    let p1 = match pt.page_table_walker.next_table(&p2[addr.p2_index()]) {
        Ok(page_table) => page_table,
        Err(PageTableWalkError::NotMapped) => {info!("PageNotMapped at P2"); return;},
        Err(PageTableWalkError::MappedToHugePage) => {
            info!("2MB frame detected.");
            return;
        }
    };

    let p1_entry = &p1[addr.p1_index()];

    if p1_entry.is_unused() {
        info!("PageNotMapped at P1"); return;
    }
    info!("p1_entry found.");
    info!("{:?}", &p1[addr.p1_index()]);

}
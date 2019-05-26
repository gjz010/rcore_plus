use crate::consts::KERNEL_OFFSET;
use crate::drivers::block::*;
use crate::drivers::net::*;
use crate::drivers::{Driver, DRIVERS, NET_DRIVERS};
use crate::memory::active_table;
use alloc::collections::BTreeMap;
use alloc::sync::Arc;
use pci::*;
use rcore_memory::{paging::PageTable, PAGE_SIZE};
use spin::Mutex;

const PCI_COMMAND: u16 = 0x04;
const PCI_CAP_PTR: u16 = 0x34;
const PCI_INTERRUPT_LINE: u16 = 0x3c;
const PCI_INTERRUPT_PIN: u16 = 0x3d;

const PCI_MSI_CTRL_CAP: u16 = 0x00;
const PCI_MSI_ADDR: u16 = 0x04;
const PCI_MSI_UPPER_ADDR: u16 = 0x08;
const PCI_MSI_DATA: u16 = 0x0C;

const PCI_CAP_ID_MSI: u8 = 0x05;

struct PortOpsImpl;

#[cfg(target_arch = "x86_64")]
use x86_64::instructions::port::Port;

#[cfg(target_arch = "x86_64")]
impl PortOps for PortOpsImpl {
    unsafe fn read8(&self, port: u16) -> u8 {
        Port::new(port).read()
    }
    unsafe fn read16(&self, port: u16) -> u16 {
        Port::new(port).read()
    }
    unsafe fn read32(&self, port: u16) -> u32 {
        Port::new(port).read()
    }
    unsafe fn write8(&self, port: u16, val: u8) {
        Port::new(port).write(val);
    }
    unsafe fn write16(&self, port: u16, val: u16) {
        Port::new(port).write(val);
    }
    unsafe fn write32(&self, port: u16, val: u32) {
        Port::new(port).write(val);
    }
}

#[cfg(target_arch = "mips")]
use crate::util::{read, write};
use alloc::vec::Vec;

#[cfg(feature = "board_malta")]
const PCI_BASE: usize = 0xbbe00000;

#[cfg(target_arch = "mips")]
impl PortOps for PortOpsImpl {
    unsafe fn read8(&self, port: u16) -> u8 {
        read(PCI_BASE + port as usize)
    }
    unsafe fn read16(&self, port: u16) -> u16 {
        read(PCI_BASE + port as usize)
    }
    unsafe fn read32(&self, port: u16) -> u32 {
        read(PCI_BASE + port as usize)
    }
    unsafe fn write8(&self, port: u16, val: u8) {
        write(PCI_BASE + port as usize, val);
    }
    unsafe fn write16(&self, port: u16, val: u16) {
        write(PCI_BASE + port as usize, val);
    }
    unsafe fn write32(&self, port: u16, val: u32) {
        write(PCI_BASE + port as usize, val);
    }
}

/// Enable the pci device and its interrupt
/// Return assigned MSI interrupt number when applicable
pub unsafe fn enable(loc: Location) -> Option<u32> {
    let ops = &PortOpsImpl;
    let am = CSpaceAccessMethod::IO;

    // 23 and lower are used
    static mut MSI_IRQ: u32 = 23;

    let orig = am.read16(ops, loc, PCI_COMMAND);
    // IO Space | MEM Space | Bus Mastering | Special Cycles | PCI Interrupt Disable
    am.write32(ops, loc, PCI_COMMAND, (orig | 0x40f) as u32);

    // find MSI cap
    let mut msi_found = false;
    let mut cap_ptr = am.read8(ops, loc, PCI_CAP_PTR) as u16;
    let mut assigned_irq = None;
    while cap_ptr > 0 {
        let cap_id = am.read8(ops, loc, cap_ptr);
        if cap_id == PCI_CAP_ID_MSI {
            // The manual Volume 3 Chapter 10.11 Message Signalled Interrupts
            // 0 is (usually) the apic id of the bsp.
            am.write32(ops, loc, cap_ptr + PCI_MSI_ADDR, 0xfee00000 | (0 << 12));
            MSI_IRQ += 1;
            let irq = MSI_IRQ;
            assigned_irq = Some(irq);
            // we offset all our irq numbers by 32
            am.write32(ops, loc, cap_ptr + PCI_MSI_DATA, irq + 32);

            // enable MSI interrupt, assuming 64bit for now
            let orig_ctrl = am.read32(ops, loc, cap_ptr + PCI_MSI_CTRL_CAP);
            am.write32(ops, loc, cap_ptr + PCI_MSI_CTRL_CAP, orig_ctrl | 0x10000);
            debug!(
                "MSI control {:#b}, enabling MSI interrupt {}",
                orig_ctrl >> 16,
                irq
            );
            msi_found = true;
            break;
        }
        debug!("PCI device has cap id {} at {:#X}", cap_id, cap_ptr);
        cap_ptr = am.read8(ops, loc, cap_ptr + 1) as u16;
    }

    if !msi_found {
        // Use PCI legacy interrupt instead
        // IO Space | MEM Space | Bus Mastering | Special Cycles
        am.write32(ops, loc, PCI_COMMAND, (orig | 0xf) as u32);
        debug!("MSI not found, using PCI interrupt");
    }

    assigned_irq
}

pub fn init_driver(dev: &PCIDevice) {
    let name = format!("enp{}s{}f{}", dev.loc.bus, dev.loc.device, dev.loc.function);
    match (dev.id.vendor_id, dev.id.device_id) {
        (0x8086, 0x100e) | (0x8086, 0x100f) | (0x8086, 0x10d3) => {
            // 0x100e
            // 82540EM Gigabit Ethernet Controller
            // 0x100f
            // 82545EM Gigabit Ethernet Controller (Copper)
            // 0x10d3
            // 82574L Gigabit Network Connection
            if let Some(BAR::Memory(addr, len, _, _)) = dev.bars[0] {
                let irq = unsafe { enable(dev.loc) };
                let vaddr = KERNEL_OFFSET + addr as usize;
                let mut current_addr = addr as usize;
                while current_addr < addr as usize + len as usize {
                    active_table().map_if_not_exists(KERNEL_OFFSET + current_addr, current_addr);
                    current_addr = current_addr + PAGE_SIZE;
                }
                let index = NET_DRIVERS.read().len();
                e1000::init(name, irq, vaddr, len as usize, index);
            }
        }
        (0x8086, 0x10fb) => {
            // 82599ES 10-Gigabit SFI/SFP+ Network Connection
            if let Some(BAR::Memory(addr, len, _, _)) = dev.bars[0] {
                let irq = unsafe { enable(dev.loc) };
                let vaddr = KERNEL_OFFSET + addr as usize;
                let mut current_addr = addr as usize;
                while current_addr < addr as usize + len as usize {
                    active_table().map_if_not_exists(KERNEL_OFFSET + current_addr, current_addr);
                    current_addr = current_addr + PAGE_SIZE;
                }
                let index = NET_DRIVERS.read().len();
                PCI_DRIVERS.lock().insert(
                    dev.loc,
                    ixgbe::ixgbe_init(name, irq, vaddr, len as usize, index),
                );
            }
        }
        (0x8086, 0x2922) => {
            // 82801IR/IO/IH (ICH9R/DO/DH) 6 port SATA Controller [AHCI mode]
            if let Some(BAR::Memory(addr, len, _, _)) = dev.bars[5] {
                let irq = unsafe { enable(dev.loc) };
                assert!(len as usize <= PAGE_SIZE);
                let vaddr = KERNEL_OFFSET + addr as usize;
                active_table().map(vaddr, addr as usize);
                PCI_DRIVERS
                    .lock()
                    .insert(dev.loc, ahci::init(irq, vaddr, len as usize));
            }
        }
        _ => {}
    }
}

pub fn detach_driver(loc: &Location) -> bool {
    match PCI_DRIVERS.lock().remove(loc) {
        Some(driver) => {
            DRIVERS
                .write()
                .retain(|dri| dri.get_id() != driver.get_id());
            NET_DRIVERS
                .write()
                .retain(|dri| dri.get_id() != driver.get_id());
            true
        }
        None => false,
    }
}

pub fn init() {
    let pci_iter = unsafe { scan_bus(&PortOpsImpl, CSpaceAccessMethod::IO) };
    for dev in pci_iter {
        println!(
            "pci: {:02x}:{:02x}.{} {:#x} {:#x} ({} {}) irq: {}:{:?}",
            dev.loc.bus,
            dev.loc.device,
            dev.loc.function,
            dev.id.vendor_id,
            dev.id.device_id,
            dev.id.class,
            dev.id.subclass,
            dev.pic_interrupt_line,
            dev.interrupt_pin,
        );
        init_driver(&dev);
    }
}

pub fn find_device(vendor: u16, product: u16) -> Option<Location> {
    let pci_iter = unsafe { scan_bus(&PortOpsImpl, CSpaceAccessMethod::IO) };
    for dev in pci_iter {
        if dev.id.vendor_id == vendor && dev.id.device_id == product {
            return Some(dev.loc);
        }
    }
    None
}
pub fn find_all_devices(vendor: u16, product: u16)->Vec<Location>{
    let pci_iter = unsafe { scan_bus(&PortOpsImpl, CSpaceAccessMethod::IO) };
    let mut v: Vec<Location>=Vec::new();
    for dev in pci_iter {
        if dev.id.vendor_id == vendor && dev.id.device_id == product {
            v.push(dev.loc);
        }
    }
    v
}
pub fn get_bar0_mem(loc: Location) -> Option<(usize, usize)> {
    unsafe { probe_function(&PortOpsImpl, loc, CSpaceAccessMethod::IO) }
        .and_then(|dev| dev.bars[0])
        .map(|bar| {
            info!("{:?}", bar);
            match bar {
            BAR::Memory(addr, len, _, _) => (addr as usize, len as usize),
            _ => unimplemented!(),
        }})
}
pub fn get_bar0_io(loc: Location) -> Option<(u16, u16)> {
    unsafe { probe_function(&PortOpsImpl, loc, CSpaceAccessMethod::IO) }
        .and_then(|dev| dev.bars[0])
        .map(|bar| {
            info!("{:?}", bar);
            match bar {
                BAR::IO(addr, len)=>(addr as u16, len as u16),
                _ => unimplemented!(),
            }})
}
lazy_static! {
    pub static ref PCI_DRIVERS: Mutex<BTreeMap<Location, Arc<Driver>>> =
        Mutex::new(BTreeMap::new());
}

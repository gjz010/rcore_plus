extern crate rcore;
extern crate alloc;
extern crate spin;
extern crate pci;
extern crate bitflags;
extern crate rcore_memory;
extern crate x86_64;
extern crate log;
use rcore::lkm::cdev::{FileOperations, CDevManager, CharDev};
use rcore::fs::{FileHandle, SeekFrom};
use rcore::rcore_fs::vfs::{FsError, Metadata, PollStatus};
use alloc::string::String;
use rcore::drivers::bus::pci as rcore_pci;
use rcore::drivers::bus::pci::enable;
use pci::BAR;
use rcore::memory::active_table;
use rcore::consts::KERNEL_OFFSET;
use rcore::drivers::bus::pci::get_bar0_io;
use rcore_memory::PAGE_SIZE;
use rcore::drivers::{Driver, DeviceType};
use rcore::rcore_fs::vfs::FsError::DeviceError;
use rcore::sync::Condvar;
use rcore::drivers::DRIVERS;
use alloc::sync::Arc;
use rcore::sync::SpinNoIrqLock as Mutex;
use core::marker::PhantomData;
use bitflags::bitflags;
use core::any::Any;
use log::info;
bitflags! {
    /// Interrupt enable flags
    struct IntEnFlags: u8 {
        const RECEIVED = 1;
        const SENT = 1 << 1;
        const ERRORED = 1 << 2;
        const STATUS_CHANGE = 1 << 3;
        // 4 to 7 are unused
    }
}

bitflags! {
    /// Line status flags
    struct LineStsFlags: u8 {
        const DATA_READY = 1;
        // 1 to 4 unknown
        const OUTPUT_EMPTY = 1 << 5;
        // 6 and 7 unknown
    }
}


use rcore::sync::mpsc::*;
use x86_64::instructions::port::Port;
pub fn register_uart(){

    use rcore_memory::paging::PageTable;
    use rcore::drivers::bus::pci::find_device;
    let loc=find_device(0x1b36, 0x0002).unwrap();
    //for loc in serials.iter {
    if let Some((addr, len)) = get_bar0_io(loc) {
        let irq = unsafe { enable(loc) };
        let mut serial_obj=PCISerial::new(irq, addr);
        serial_obj.init();
        let serial:Arc<Driver>=Arc::new(serial_obj);
        DRIVERS.write().push(Arc::clone(&serial));

        let mut cdev=CDevManager::get().write();
        cdev.registerDevice(16, CharDev{
            parent_module: None,
            file_op: Arc::new(PCISerialOps(serial))
        });
    }

    //}
}

pub type SerialPort=Port<u8>;


// We make no guarantee about the serial.
pub struct PCISerial{
    //can_write: Condvar,
    biglock_read: Mutex<()>,
    biglock_write: Mutex<()>,
    irq: Option<u32>,
    data: SerialPort,
    int_en: SerialPort,
    fifo_ctrl: SerialPort,
    line_ctrl: SerialPort,
    modem_ctrl: SerialPort,
    line_sts: SerialPort,
    //read_sender_isr: Mutex<Sender<u8>>, // ISR does not need this!
    //read_receiver_proc: Mutex<Receiver<u8>>
}
impl PCISerial{
    pub fn new(irq: Option<u32>, base: u16)->Self{
        let (isr, proc)=channel::<u8>();
        PCISerial{
            //can_write: Condvar::new(),
            biglock_read: Mutex::new(()),
            biglock_write: Mutex::new(()),
            irq: irq.clone(),
            data: unsafe{ SerialPort::new(base)},
            int_en: unsafe{ SerialPort::new(base+1)},
            fifo_ctrl: unsafe{ SerialPort::new(base+2)},
            line_ctrl: unsafe{ SerialPort::new(base+3)},
            modem_ctrl: unsafe{ SerialPort::new(base+4)},
            line_sts: unsafe{ SerialPort::new(base+5)},
            //read_sender_isr: Mutex::new(isr),
            //read_receiver_proc: Mutex::new(proc)
        }
    }
    fn init(&mut self){
        unsafe {
            self.int_en.write(0x00);    // Disable all interrupts
            self.line_ctrl.write(0x80);    // Enable DLAB (set baud rate divisor)
            self.data.write(0x03);    // Set divisor to 3 (lo byte) 38400 baud
            self.int_en.write(0x00);    //                  (hi byte)
            self.line_ctrl.write(0x03);    // 8 bits, no parity, one stop bit
            self.fifo_ctrl.write(0xC7);    // Enable FIFO, clear them, with 14-byte threshold
            self.modem_ctrl.write(0x0B);    // IRQs enabled, RTS/DSR set
        }
        // and no interrupt here.
    }
    fn line_sts(&self) -> LineStsFlags {
        unsafe { LineStsFlags::from_bits_truncate(self.line_sts.read()) }
    }
    fn can_write(&self)->bool{
        self.line_sts().contains(LineStsFlags::OUTPUT_EMPTY)
    }
    fn can_read(&self)->bool{
        self.line_sts().contains(LineStsFlags::DATA_READY)
    }
    pub fn read_byte(&self)->u8{
        let lock=self.biglock_read.lock();
        while !self.can_read(){
            //spin and spin.
        }
        unsafe {self.data.read()}
    }
    // We use a spin-write here and hope that writing does not take much time.
    pub fn write_byte(&mut self, data: u8){
        let lock=self.biglock_write.lock();
        while !self.can_write(){
            //spin and spin.
        }
        unsafe {self.data.write(data);}
    }
}
pub struct PCISerialOps(Arc<Driver>);
// No I don't think a driver should do anything else than representing itself. and handling interrupts.
impl Driver for PCISerial{
    fn try_handle_interrupt(&self, irq: Option<u32>) -> bool {
        // We do nothing here since we don't need interrupt, at least by now.
        false
    }

    fn device_type(&self) -> DeviceType {
        DeviceType::Input
    }

    fn get_id(&self) -> String {
        String::from("pci_serial")
    }
    fn as_any_ref(&self)->&Any{
        self
    }
}
impl FileOperations for PCISerialOps{
    fn open(&self) -> usize {
        info!("PCISerial opened.");
        0
    }

    fn read(&self, fh: &mut FileHandle, buf: &mut [u8]) -> Result<usize, FsError> {
        let serial=self.0.as_any_ref().downcast_ref::<PCISerial>().unwrap();
        for chr in buf.iter_mut(){
            *chr=serial.read_byte();
        }
        Ok(buf.len())
    }

    fn read_at(&self, fh: &mut FileHandle, offset: usize, buf: &mut [u8]) -> Result<usize, FsError> {
        Err(FsError::NotSupported)
    }

    fn write(&self, fh: &mut FileHandle, buf: &[u8]) -> Result<usize, FsError> {
        let serial=self.0.as_any_ref().downcast_ref::<PCISerial>().unwrap();
        for chr in buf.iter(){
            (unsafe {&mut *(serial as *const PCISerial as *mut PCISerial)}).write_byte(*chr);
        }
        Ok(buf.len())
    }

    fn write_at(&self, fh: &mut FileHandle, offset: usize, buf: &[u8]) -> Result<usize, FsError> {
        Err(FsError::NotSupported)
    }

    fn seek(&self, fh: &mut FileHandle, pos: SeekFrom) -> Result<u64, FsError> {
        Err(FsError::NotSupported)
    }

    fn set_len(&self, fh: &mut FileHandle, len: u64) -> Result<(), FsError> {
        Err(FsError::NotSupported)
    }

    fn sync_all(&self, fh: &mut FileHandle) -> Result<(), FsError> {
        Err(FsError::NotSupported)
    }

    fn sync_data(&self, fh: &mut FileHandle) -> Result<(), FsError> {
        Err(FsError::NotSupported)
    }

    fn metadata(&self, fh: &FileHandle) -> Result<Metadata, FsError> {
        Err(FsError::NotSupported)
    }

    fn read_entry(&self, fh: &mut FileHandle) -> Result<String, FsError> {
        Err(FsError::NotSupported)
    }

    fn poll(&self, fh: &FileHandle) -> Result<PollStatus, FsError> {
        Err(FsError::NotSupported)
    }

    fn io_control(&self, fh: &FileHandle, cmd: u32, arg: usize) -> Result<(), FsError> {
        Err(FsError::NotSupported)
    }

    fn close(&self, data: usize) {

    }
}
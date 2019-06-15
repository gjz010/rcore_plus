extern crate alloc;
extern crate bcm2837;
extern crate log;
extern crate rcore;
extern crate rcore_fs;
extern crate rcore_memory;
extern crate spin;
extern crate isomorphic_drivers;
use bcm2837::{pwm, dma};
use core::mem;
use core::slice;
use core::time::Duration;
use log::*;
use rcore::arch::board::mailbox;
use rcore::thread;

use alloc::boxed::Box;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::any::Any;
use rcore::fs::vfs::INodeContainer;
use rcore::lkm::device::{DeviceFileProvider, DeviceHandle, OverlaidINode};
use rcore_fs::vfs::{FsError, PollStatus};
use rcore_memory::memory_set::handler::MemoryHandler;
use spin::Mutex;

use rcore::drivers::provider;
use isomorphic_drivers::provider::Provider;
#[derive(Default)]
pub struct AudioDeviceHandle {
    buf: Mutex<Vec<u8>>
}

pub struct AudioDriver(Arc<DeviceHandle>);

impl AudioDriver {
    fn new() -> Self {
        AudioDriver(Arc::new(AudioDeviceHandle::default()))
    }
    pub fn init() {
        let audio = Self::new();
        let driver = Box::new(audio);
        let mut cdev = rcore::lkm::device::CDevManager::get().write();
        cdev.registerDevice(233, driver);
    }
}

impl DeviceFileProvider for AudioDriver {
    fn open(&self, minor: usize) -> Option<Arc<DeviceHandle>> {
        if minor == 0 {
            Some(Arc::clone(&self.0))
        } else {
            info!("Bad minor!");
            None
        }
    }
}

impl DeviceHandle for AudioDeviceHandle {
    fn read_at(
        &self,
        offset: usize,
        buf: &mut [u8],
        original_file: Option<&OverlaidINode>,
    ) -> Result<usize, FsError> {
        Err(FsError::NotSupported)
    }

    fn write_at(
        &self,
        offset: usize,
        buf: &[u8],
        original_file: Option<&OverlaidINode>,
    ) -> Result<usize, FsError> {
        let tmp = &mut Vec::<u8>::from(buf);
        self.buf.lock().append(tmp);
        Ok(buf.len())
    }

    fn poll(&self, original_file: Option<&OverlaidINode>) -> Result<PollStatus, FsError> {
        Ok(PollStatus {
            read: false,
            write: true,
            error: false,
        })
    }

    fn sync_data(&self, original_file: Option<&OverlaidINode>) -> Result<(), FsError> {
        Ok(())
    }

    fn io_control(
        &self,
        cmd: u32,
        data: usize,
        original_file: Option<&OverlaidINode>,
    ) -> Result<(), FsError> {
        if cmd == 0 {
            // clear buffer and get ready for receiving audio data
            self.buf.lock().clear();
        } else if cmd == 1 {
            info!("dsp ioctl REQ = 1, DMA output");
            let buflen = self.buf.lock().len();
            let chunk_size = 1000000;
            info!("buf len: {}\n", buflen);

            let (vaddr0, paddr0) = provider::Provider::alloc_dma(chunk_size * 4);
            let (mut cb0_vaddr, mut cb0_paddr) = provider::Provider::alloc_dma(32);
            info!("buf vaddr: {}, buf paddr: {}\n", vaddr0, paddr0);
            info!("cb vaddr: {}, cb paddr: {}\n", cb0_vaddr, cb0_paddr);

            // copy data
            let mut dma_buf_ptr = vaddr0 as *mut u32;
            let mut buf_lock = self.buf.lock();
            let max_len = chunk_size / 2;
            for i in 0..max_len {
                let u32_data = (buf_lock[i] as u32) << 4;
                unsafe { *dma_buf_ptr = u32_data; }
                unsafe { dma_buf_ptr = dma_buf_ptr.offset(1); }
                unsafe { *dma_buf_ptr = u32_data; }
                unsafe { dma_buf_ptr = dma_buf_ptr.offset(1); }
            }

            // start pwm
            let mut pwm_output = pwm::PWMOutput::new();
            pwm_output.start(5669, true);
            pwm_output.dma_start();

            let mut dma_handler = dma::DMA::new(5, chunk_size, cb0_vaddr, cb0_paddr, vaddr0, paddr0);
            dma_handler.start();

        } else if cmd == 2 {
            info!("dsp ioctl REQ = 2, PWM output");
            let buflen = self.buf.lock().len();
            let mut pwm_output = pwm::PWMOutput::new();
            pwm_output.start(5669, true);
            // pwm_output.start(62500, true);
            let mut buf_lock = self.buf.lock();
            while true {
                for i in 0..buflen {
                    let u32_data = (buf_lock[i] as u32) << 4;
                    pwm_output.write_fifo(u32_data);
                    pwm_output.write_fifo(u32_data);
                }
            }
            info!("finish pwm output: {}\n", buflen);
        } else if cmd == 3 {
            info!("dsp ioctl REQ = 3, stop DMA");
            let (vaddr0, paddr0) = provider::Provider::alloc_dma(128 * 4);
            let (mut cb0_vaddr, mut cb0_paddr) = provider::Provider::alloc_dma(32);
            let mut dma_handler = dma::DMA::new(5, 128, cb0_vaddr, cb0_paddr, vaddr0, paddr0);
            dma_handler.stop();
        }
        Ok(())
    }

    fn as_any_ref(&self) -> &Any {
        self
    }

    fn mmap(
        &self,
        start_addr: usize,
        end_addr: usize,
        prot: usize,
        offset: usize,
        original_file: Option<&OverlaidINode>,
    ) -> Result<Box<MemoryHandler>, FsError> {
        Err(FsError::NotSupported)
    }

    fn overrideSymbolLink(
        &self,
        original_file: Option<&OverlaidINode>,
    ) -> Option<Arc<INodeContainer>> {
        None
    }
}

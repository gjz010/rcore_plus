//! Implement INode for Stdin & Stdout

use alloc::{collections::vec_deque::VecDeque, string::String, sync::Arc, vec::Vec};
use core::any::Any;
use spin::RwLock;

use rcore_fs::vfs::*;
use rcore_fs_sfs;

use super::ioctl::*;
use crate::drivers::provider;
use isomorphic_drivers::provider::Provider;
use crate::sync::Condvar;
use crate::sync::SpinNoIrqLock as Mutex;

use bcm2837::gpio;
use bcm2837::pwm_sound_device;
use bcm2837::pwm;
use bcm2837::dma;
use bcm2837::timer;

#[derive(Default)]
pub struct Stdin {
    buf: Mutex<VecDeque<char>>,
    pub pushed: Condvar,
}

impl Stdin {
    pub fn push(&self, c: char) {
        self.buf.lock().push_back(c);
        self.pushed.notify_one();
    }
    pub fn pop(&self) -> char {
        #[cfg(feature = "board_k210")]
        loop {
            // polling
            let c = crate::arch::io::getchar();
            if c != '\0' {
                return c;
            }
        }
        #[cfg(not(feature = "board_k210"))]
        loop {
            let mut buf_lock = self.buf.lock();
            match buf_lock.pop_front() {
                Some(c) => return c,
                None => {
                    self.pushed.wait(buf_lock);
                }
            }
        }
    }
    pub fn can_read(&self) -> bool {
        return self.buf.lock().len() > 0;
    }
}

#[derive(Default)]
pub struct Stdout;

#[derive(Default)]
pub struct Dsp {
    buf: Mutex<Vec<u8>>
}

#[derive(Default)]
pub struct GPIOOutput {
    pin: RwLock<u8>
}

impl GPIOOutput {
    fn new(init_pin: u8) -> Self {
        GPIOOutput {
            pin: RwLock::new(init_pin)
        }
    }
}

pub const STDIN_ID: usize = 0;
pub const STDOUT_ID: usize = 1;
pub const STDERR_ID: usize = 2;
pub const GPIO_ID: usize = 3;
pub const DSP_ID: usize = 4;

lazy_static! {
    pub static ref STDIN: Arc<Stdin> = Arc::new(Stdin::default());
    pub static ref STDOUT: Arc<Stdout> = Arc::new(Stdout::default());
    pub static ref GPIO: Arc<GPIOOutput> = Arc::new(GPIOOutput::new(0));
    pub static ref DSP: Arc<Dsp> = Arc::new(Dsp::default());
}

// TODO: better way to provide default impl?
macro_rules! impl_inode {
    () => {
        fn metadata(&self) -> Result<Metadata> { Err(FsError::NotSupported) }
        fn set_metadata(&self, _metadata: &Metadata) -> Result<()> { Ok(()) }
        fn sync_all(&self) -> Result<()> { Ok(()) }
        fn sync_data(&self) -> Result<()> { Ok(()) }
        fn resize(&self, _len: usize) -> Result<()> { Err(FsError::NotSupported) }
        fn create(&self, _name: &str, _type_: FileType, _mode: u32) -> Result<Arc<INode>> { Err(FsError::NotDir) }
        fn unlink(&self, _name: &str) -> Result<()> { Err(FsError::NotDir) }
        fn link(&self, _name: &str, _other: &Arc<INode>) -> Result<()> { Err(FsError::NotDir) }
        fn move_(&self, _old_name: &str, _target: &Arc<INode>, _new_name: &str) -> Result<()> { Err(FsError::NotDir) }
        fn find(&self, _name: &str) -> Result<Arc<INode>> { Err(FsError::NotDir) }
        fn get_entry(&self, _id: usize) -> Result<String> { Err(FsError::NotDir) }
        fn fs(&self) -> Arc<FileSystem> { unimplemented!() }
        fn as_any_ref(&self) -> &Any { self }
    };
}

impl INode for Stdin {
    fn read_at(&self, offset: usize, buf: &mut [u8]) -> Result<usize> {
        buf[0] = self.pop() as u8;
        Ok(1)
    }
    fn write_at(&self, _offset: usize, _buf: &[u8]) -> Result<usize> {
        unimplemented!()
    }
    fn poll(&self) -> Result<PollStatus> {
        Ok(PollStatus {
            read: self.can_read(),
            write: false,
            error: false,
        })
    }
    fn io_control(&self, cmd: u32, data: usize) -> Result<()> {
        match cmd as usize {
            TCGETS | TIOCGWINSZ | TIOCSPGRP => {
                // pretend to be tty
                Ok(())
            },
            TIOCGPGRP => {
                // pretend to be have a tty process group
                // TODO: verify pointer
                unsafe {
                    *(data as *mut u32) = 0
                };
                Ok(())
            }
            _ => Err(FsError::NotSupported)
        }
    }
    impl_inode!();
}


impl INode for Stdout {
    fn read_at(&self, _offset: usize, _buf: &mut [u8]) -> Result<usize> {
        unimplemented!()
    }
    fn write_at(&self, _offset: usize, buf: &[u8]) -> Result<usize> {
        use core::str;
        //we do not care the utf-8 things, we just want to print it!
        let s = unsafe { str::from_utf8_unchecked(buf) };
        print!("{}", s);
        Ok(buf.len())
    }
    fn poll(&self) -> Result<PollStatus> {
        Ok(PollStatus {
            read: false,
            write: true,
            error: false,
        })
    }
    fn io_control(&self, cmd: u32, data: usize) -> Result<()> {
        match cmd as usize {
            TCGETS | TIOCGWINSZ | TIOCSPGRP => {
                // pretend to be tty
                Ok(())
            },
            TIOCGPGRP => {
                // pretend to be have a tty process group
                // TODO: verify pointer
                unsafe {
                    *(data as *mut u32) = 0
                };
                Ok(())
            }
            _ => Err(FsError::NotSupported)
        }
    }
    impl_inode!();
}

impl INode for Dsp {
    fn read_at(&self, _offset: usize, _buf: &mut [u8]) -> Result<usize> {
        unimplemented!()
    }
    fn write_at(&self, _offset: usize, buf: &[u8]) -> Result<usize> {
        let tmp = &mut Vec::<u8>::from(buf);
        self.buf.lock().append(tmp);
        Ok(buf.len())
    }
    fn poll(&self) -> Result<PollStatus> {
        Ok(PollStatus {
            read: false,
            write: true,
            error: false,
        })
    }

    fn io_control(&self, request: u32, data: usize) -> Result<()> {
        if request == 0 {
            // clear buffer and get ready for receiving audio data
            self.buf.lock().clear();
        } else if request == 1 {
            print!("dsp ioctl REQ = 1, DMA output");
            let buflen = self.buf.lock().len();
            let chunk_size = 1000000;
            print!("buf len: {}\n", buflen);

            let (vaddr0, paddr0) = provider::Provider::alloc_dma(chunk_size * 4);
            let (mut cb0_vaddr, mut cb0_paddr) = provider::Provider::alloc_dma(32);
            print!("buf vaddr: {}, buf paddr: {}\n", vaddr0, paddr0);
            print!("cb vaddr: {}, cb paddr: {}\n", cb0_vaddr, cb0_paddr);

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

        } else if request == 2 {
            print!("dsp ioctl REQ = 2, PWM output");
            let buflen = self.buf.lock().len();
            let mut pwm_output = pwm::PWMOutput::new();
            pwm_output.start(5669, true);
            // pwm_output.start(62500, true);
            let mut buf_lock = self.buf.lock();
            while true {
                for i in 0..buflen {
                    let u32_data = (buf_lock[i] as u32) << 4;
                    pwm_output.writeFIFO(u32_data);
                    pwm_output.writeFIFO(u32_data);
                }
            }
            print!("finish pwm output: {}\n", buflen);
        } else if request == 3 {
            print!("dsp ioctl REQ = 3, stop DMA");
            let (vaddr0, paddr0) = provider::Provider::alloc_dma(128 * 4);
            let (mut cb0_vaddr, mut cb0_paddr) = provider::Provider::alloc_dma(32);
            let mut dma_handler = dma::DMA::new(5, 128, cb0_vaddr, cb0_paddr, vaddr0, paddr0);
            dma_handler.stop();
        }
        Ok(())
    }
    impl_inode!();
}


impl INode for GPIOOutput {
    fn read_at(&self, _offset: usize, _buf: &mut [u8]) -> Result<usize> {
        unimplemented!()
    }
    fn write_at(&self, _offset: usize, buf: &[u8]) -> Result<usize> {
        use core::str;
        let mut my_gpio = gpio::Gpio::<gpio::Uninitialized>::new(*self.pin.read()).into_output();
        my_gpio.set();
        Ok(0)
    }
    fn poll(&self) -> Result<PollStatus> {
        Ok(PollStatus {
            read: false,
            write: false,
            error: false,
        })
    }
    fn io_control(&self, request: u32, data: usize) -> Result<()> {
        if (request > 53) {
            warn!("pin id > 53!");
            return Err(FsError::NotSupported);
        }
        let mut pin = self.pin.write();
        *pin = request as u8;
        Ok(())
    }
    impl_inode!();
}

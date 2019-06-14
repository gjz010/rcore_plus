//! Implement Device

use rcore_fs::dev::*;
use spin::RwLock;

#[cfg(target_arch = "x86_64")]
use crate::arch::driver::ide;

use crate::sync::SpinNoIrqLock as Mutex;
use alloc::sync::Arc;
use rcore_fs::vfs::FsError::DeviceError;
use rcore_fs::vfs::INode;

pub struct MemBuf(RwLock<&'static mut [u8]>);

impl MemBuf {
    pub unsafe fn new(begin: unsafe extern "C" fn(), end: unsafe extern "C" fn()) -> Self {
        use core::slice;
        MemBuf(RwLock::new(slice::from_raw_parts_mut(
            begin as *mut u8,
            end as usize - begin as usize,
        )))
    }
}

impl Device for MemBuf {
    fn read_at(&self, offset: usize, buf: &mut [u8]) -> Result<usize> {
        let slice = self.0.read();
        let len = buf.len().min(slice.len() - offset);
        buf[..len].copy_from_slice(&slice[offset..offset + len]);
        Ok(len)
    }
    fn write_at(&self, offset: usize, buf: &[u8]) -> Result<usize> {
        let mut slice = self.0.write();
        let len = buf.len().min(slice.len() - offset);
        slice[offset..offset + len].copy_from_slice(&buf[..len]);
        Ok(len)
    }
    fn sync(&self) -> Result<()> {
        Ok(())
    }
}

#[cfg(target_arch = "x86_64")]
impl BlockDevice for ide::IDE {
    const BLOCK_SIZE_LOG2: u8 = 9;
    fn read_at(&self, block_id: usize, buf: &mut [u8]) -> Result<()> {
        use core::slice;
        assert!(buf.len() >= ide::BLOCK_SIZE);
        let buf =
            unsafe { slice::from_raw_parts_mut(buf.as_ptr() as *mut u32, ide::BLOCK_SIZE / 4) };
        self.read(block_id as u64, 1, buf).map_err(|_| DevError)?;
        Ok(())
    }
    fn write_at(&self, block_id: usize, buf: &[u8]) -> Result<()> {
        use core::slice;
        assert!(buf.len() >= ide::BLOCK_SIZE);
        let buf = unsafe { slice::from_raw_parts(buf.as_ptr() as *mut u32, ide::BLOCK_SIZE / 4) };
        self.write(block_id as u64, 1, buf).map_err(|_| DevError)?;
        Ok(())
    }
    fn sync(&self) -> Result<()> {
        Ok(())
    }
}

pub struct BlockINodeReader(pub Arc<INode>);

impl BlockDevice for BlockINodeReader {
    const BLOCK_SIZE_LOG2: u8 = 9;

    fn read_at(&self, block_id: usize, buf: &mut [u8]) -> Result<()> {
        info!(
            "BlockINodeReader::read block_id={} buf.len()={}",
            block_id,
            buf.len()
        );
        let block_size = 1 << Self::BLOCK_SIZE_LOG2;
        if buf.len() != block_size {
            return Err(DevError);
        }
        if self
            .0
            .read_at(block_id * block_size, buf)
            .map_err(|_| DevError)?
            != block_size
        {
            return Err(DevError);
        }
        Ok(())
    }

    fn write_at(&self, block_id: usize, buf: &[u8]) -> Result<()> {
        info!(
            "BlockINodeReader::write block_id={} buf.len()={}",
            block_id,
            buf.len()
        );
        let block_size = 1 << Self::BLOCK_SIZE_LOG2;
        if buf.len() != block_size {
            return Err(DevError);
        }
        if self
            .0
            .write_at(block_id * block_size, buf)
            .map_err(|_| DevError)?
            != block_size
        {
            return Err(DevError);
        }
        Ok(())
    }

    fn sync(&self) -> Result<()> {
        self.0.sync_data().map_err(|_| DevError)
    }
}

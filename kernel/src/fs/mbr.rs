use crate::fs::vfs::INodeContainer;
/// Implementing MBR table for given device provider.
/// The provider provides the original block device, but with root device and partitioned.
/// This device does not support mmap. You may need to implement your own cache layer.
use crate::lkm::device::{DeviceFileProvider, DeviceHandle, OverlaidINode};
use crate::sync::SpinNoIrqLock as Mutex;
use alloc::boxed::Box;
use alloc::sync::Arc;
use core::any::Any;
use rcore_fs::dev::DevError;
use rcore_fs::vfs::{FsError, PollStatus};
use rcore_memory::memory_set::handler::MemoryHandler;

type MBRList = [Option<Arc<DeviceHandle>>; 4];
const BLOCK_SIZE: usize = 512; // This is the block size in the world.
struct MBRPartition {
    source: Arc<DeviceHandle>,
    table: Arc<MBRPartitionTable>,
    start_block: usize,
    end_block: usize,
}
pub struct MBRPartitionTable {
    source: Box<DeviceFileProvider>,
    mbr_cache: Mutex<MBRList>, //Only supporting four main partitions.
}

impl MBRPartitionTable {
    pub fn new(source: Box<DeviceFileProvider>) -> Arc<MBRPartitionTable> {
        let table = MBRPartitionTable {
            source: source,
            mbr_cache: Mutex::new([None, None, None, None]),
        };
        let result = Arc::new(table);
        result.load_partitions().unwrap();
        result
    }
    pub fn load_partitions(self: &Arc<MBRPartitionTable>) -> Result<(), FsError> {
        let mut section: [u8; BLOCK_SIZE] = [0; 512];
        let source = self.source.open(0).ok_or(FsError::DeviceError)?;
        source.read_at(0, &mut section, None)?;
        if section[510] != 0x55 || section[511] != 0xAA {
            info!("The first section is not an MBR section!");
            info!("Maybe you are working on qemu using raw image.");
            info!("Change the -sd argument to raspibian.img.");
            return Ok(());
        }
        let mut mbr_cache = self.mbr_cache.lock();
        let mut start_pos = 446; // start position of the partion table
        for entry in 0..4 {
            info!("Partion entry #{}: ", entry);
            let partion_type = section[start_pos + 0x4];
            fn partion_type_map(partion_type: u8) -> &'static str {
                match partion_type {
                    0x00 => "Empty",
                    0x0c => "FAT32",
                    0x83 => "Linux",
                    0x82 => "Swap",
                    _ => "Not supported",
                }
            }
            info!("{:^14}", partion_type_map(partion_type));
            if partion_type != 0x00 {
                let start_section: u32 = (section[start_pos + 0x8] as u32)
                    | (section[start_pos + 0x9] as u32) << 8
                    | (section[start_pos + 0xa] as u32) << 16
                    | (section[start_pos + 0xb] as u32) << 24;
                let total_section: u32 = (section[start_pos + 0xc] as u32)
                    | (section[start_pos + 0xd] as u32) << 8
                    | (section[start_pos + 0xe] as u32) << 16
                    | (section[start_pos + 0xf] as u32) << 24;
                info!(
                    " start section no. = {}, a total of {} sections in use.",
                    start_section, total_section
                );
                mbr_cache[entry] = Some(Arc::new(MBRPartition {
                    source: Arc::clone(&source),
                    table: Arc::clone(self),
                    start_block: start_section as usize,
                    end_block: (start_section + total_section - 1) as usize,
                }));
            }
            info!("");

            start_pos += 16;
        }
        Ok(())
    }
}
impl DeviceFileProvider for Arc<MBRPartitionTable> {
    fn open(&self, minor: usize) -> Option<Arc<DeviceHandle>> {
        if minor > 4 {
            None
        } else {
            let origin = self.source.open(0)?;
            if minor == 0 {
                Some(origin)
            } else {
                let mbr_cache = self.mbr_cache.lock();
                let part = mbr_cache[minor - 1].as_ref()?;
                Some(Arc::clone(part))
            }
        }
    }
}

impl DeviceHandle for MBRPartition {
    fn read_at(
        &self,
        offset: usize,
        buf: &mut [u8],
        original_file: Option<&OverlaidINode>,
    ) -> Result<usize, FsError> {
        let lower_bound = self.start_block * BLOCK_SIZE;
        let upper_bound = self.end_block * BLOCK_SIZE;
        if (offset & (BLOCK_SIZE - 1)) != 0_
            || buf.len() != BLOCK_SIZE
            || offset + lower_bound >= upper_bound
        {
            Err(FsError::InvalidParam)
        } else {
            self.source
                .read_at(offset + lower_bound, buf, original_file)
        }
    }

    fn write_at(
        &self,
        offset: usize,
        buf: &[u8],
        original_file: Option<&OverlaidINode>,
    ) -> Result<usize, FsError> {
        let lower_bound = self.start_block * BLOCK_SIZE;
        let upper_bound = self.end_block * BLOCK_SIZE;
        if (offset & (BLOCK_SIZE - 1)) != 0_
            || buf.len() != BLOCK_SIZE
            || offset + lower_bound >= upper_bound
        {
            Err(FsError::InvalidParam)
        } else {
            self.source
                .write_at(offset + lower_bound, buf, original_file)
        }
    }

    fn poll(&self, original_file: Option<&OverlaidINode>) -> Result<PollStatus, FsError> {
        Ok(PollStatus {
            read: true,
            write: true,
            error: true,
        })
    }

    fn sync_data(&self, original_file: Option<&OverlaidINode>) -> Result<(), FsError> {
        self.source.sync_data(original_file)
    }

    fn io_control(
        &self,
        cmd: u32,
        data: usize,
        original_file: Option<&OverlaidINode>,
    ) -> Result<(), FsError> {
        self.source.io_control(cmd, data, original_file)
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

use alloc::sync::Arc;
use crate::drivers::BlockDriver;
use crate::rcore_fs::dev::BlockDevice;
use crate::rcore_fs::dev;
use crate::rcore_fs::dev::DevError;
use spin::RwLock;
use alloc::collections::btree_map::BTreeMap;
use crate::lkm::cdev::FileOperations;
use crate::fs::{FileHandle, SeekFrom};
use crate::rcore_fs::vfs::{FsError, Metadata, PollStatus};
use alloc::string::String;
use core::any::Any;
use alloc::slice;

// Works as a decorator to original block driver, splitting the block-driver into many.
pub struct PartitionBlockDriver{
    driver: BlockDriver,
    current_partitions: RwLock<BTreeMap<usize, Arc<BlockDriverPart>>>
}
impl PartitionBlockDriver{
    pub fn new(block: BlockDriver)->PartitionBlockDriver{
        let pbd=PartitionBlockDriver{
            driver: block,
            current_partitions: RwLock::new(BTreeMap::new())
        };
        pbd.load_partition_table();
        pbd
    }
    pub fn load_partition_table(&self){
        let mut partitions=self.current_partitions.write();
        partitions.clear();
        // TODO: Add entire device
        partitions.insert(0, Arc::new(BlockDriverPart{
            start_block: 0,
            end_block: (-1) as isize as usize
        }));
        let mut section: [u8; 512] = [0; 512];
        let buf = unsafe { slice::from_raw_parts_mut(section.as_ptr() as *mut u32, 512 / 4) };
        self.driver.read_at(0, &mut section).unwrap();
        if section[510] != 0x55 || section[511] != 0xAA {
            return;
        }
        let mut start_pos = 446; // start position of the partion table
        for entry in 0..4 {
            //info!("Partion entry #{}: ", entry);
            let partion_type = section[start_pos + 0x4];
            fn partion_type_map(partion_type : u8) -> &'static str {
                match partion_type {
                    0x00 => "Empty",
                    0x0c => "FAT32",
                    0x83 => "Linux",
                    0x82 => "Swap",
                    _ => "Not supported"
                }
            }
            //info!("{:^14}", partion_type_map(partion_type));
            if partion_type != 0x00 {
                let start_section: u32 = (section[start_pos + 0x8] as u32)
                    | (section[start_pos + 0x9] as u32) << 8
                    | (section[start_pos + 0xa] as u32) << 16
                    | (section[start_pos + 0xb] as u32) << 24;
                let total_section: u32= (section[start_pos + 0xc] as u32)
                    | (section[start_pos + 0xd] as u32) << 8
                    | (section[start_pos + 0xe] as u32) << 16
                    | (section[start_pos + 0xf] as u32) << 24;
                let part=BlockDriverPart{
                    start_block: start_section as usize,
                    end_block: (start_section+total_section-1) as usize
                };
                partitions.insert(entry, Arc::new(part));

            }
            //info!("");
            start_pos += 16;
        }

    }

}
impl FileOperations for PartitionBlockDriver{
    fn open(&self, minor:usize) -> usize {
        let table=self.current_partitions.read();
        let partition=table.get(&minor);
        if let Some(dev)=partition{
            Arc::into_raw(Arc::clone(dev)) as usize
        }else{
            panic!("Bad minor!");

        }
    }

    fn read(&self, fh: &mut FileHandle, buf: &mut [u8]) -> Result<usize, FsError> {
        Err(FsError::NotSupported)
    }

    fn read_at(&self, fh: &mut FileHandle, offset: usize, buf: &mut [u8]) -> Result<usize, FsError> {
        let partinfo=unsafe {&*(fh.user_data as *const BlockDriverPart)};
        if ((offset & ((1<<self.driver.block_size_log2())-1)) ==0) && buf.len() == 1<<self.driver.block_size_log2(){
            let block_id=offset>>self.driver.block_size_log2();
            if block_id+partinfo.start_block>=partinfo.start_block && block_id+partinfo.start_block<=partinfo.end_block{
                self.driver.read_at(block_id, buf).unwrap();
                Ok(buf.len())
            }else{
                // Boundary overflow. try again.
                Err(FsError::InvalidParam)
            }


        }else{
            //unaligned read. try again.
            Err(FsError::InvalidParam)
        }
    }

    fn write(&self, fh: &mut FileHandle, buf: &[u8]) -> Result<usize, FsError> {
        Err(FsError::NotSupported)
    }

    fn write_at(&self, fh: &mut FileHandle, offset: usize, buf: &[u8]) -> Result<usize, FsError> {
        let partinfo=unsafe {&*(fh.user_data as *const BlockDriverPart)};
        if ((offset & ((1<<self.driver.block_size_log2())-1)) ==0) && buf.len() == 1<<self.driver.block_size_log2(){
            let block_id=offset>>self.driver.block_size_log2();
            if block_id+partinfo.start_block>=partinfo.start_block && block_id+partinfo.start_block<=partinfo.end_block{
                self.driver.write_at(block_id, buf).unwrap();
                Ok(buf.len())
            }else{
                // Boundary overflow. try again.
                Err(FsError::InvalidParam)
            }


        }else{
            //unaligned read. try again.
            Err(FsError::InvalidParam)
        }
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
        // TODO: reload partitions
        Err(FsError::NotSupported)
    }

    fn close(&self, data: usize) {
        unsafe{Arc::from_raw(data as *const BlockDriverPart);}
    }

    fn as_any_ref(&self) -> &Any {
        self
    }
}


pub struct BlockDriverPart{
    start_block: usize,
    end_block: usize
}

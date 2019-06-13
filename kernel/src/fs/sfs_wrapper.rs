use crate::lkm::fs::FileSystemType;
use crate::syscall;
use crate::syscall::Syscall;
use alloc::sync::Arc;
use rcore_fs::dev::block_cache::BlockCache;
use rcore_fs::vfs::{FileSystem, FsError};

pub struct SFSWrapper;

impl FileSystemType for SFSWrapper {
    fn mount(
        &self,
        syscall: &mut Syscall,
        source: &str,
        flags: u64,
        data: usize,
    ) -> Result<Arc<FileSystem>, FsError> {
        info!("Start mounting");
        let proc = syscall.process();
        info!("Find device {}", source);
        let (inode, rdev) = crate::lkm::device::CDevManager::findDevice(source, &proc.cwd)?;
        let major = crate::lkm::device::dev_major(rdev as u64) as usize;
        let minor = crate::lkm::device::dev_minor(rdev as u64) as usize;
        info!("get cdev {} {}", major, minor);
        let cdev = crate::lkm::device::CDevManager::get().read();
        let new_inode = cdev
            .openINode(&inode.inode, major, minor)
            .ok_or(FsError::NoDevice)?;
        let inode_reader = crate::fs::device::BlockINodeReader(Arc::new(new_inode));
        info!("sfs");
        Ok((rcore_fs_sfs::SimpleFileSystem::open(Arc::new(BlockCache::new(inode_reader, 0x100)))?))
    }
}

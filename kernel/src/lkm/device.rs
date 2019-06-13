use crate::fs::vfs::PathResolveResult;
use crate::fs::vfs::{INodeContainer, PathConfig};
use crate::memory::GlobalFrameAlloc;
use crate::process::structs::INodeForMap;
use alloc::boxed::Box;
use alloc::collections::btree_map::BTreeMap;
use alloc::string::String;
use alloc::sync::Arc;
use core::any::Any;
use rcore_fs::vfs::Result;
use rcore_fs::vfs::{FileSystem, FileType, FsError, INode, Metadata, PollStatus};
use rcore_memory::memory_set::handler::{File, MemoryHandler};
use spin::RwLock;

/// Represents an overlay handle.
/// This handle supports fancier file system operations.
pub trait DeviceHandle: Send + Sync {
    /// Read bytes at `offset` into `buf`, return the number of bytes read.
    fn read_at(
        &self,
        offset: usize,
        buf: &mut [u8],
        original_file: Option<&OverlaidINode>,
    ) -> Result<usize>;

    /// Write bytes at `offset` from `buf`, return the number of bytes written.
    fn write_at(
        &self,
        offset: usize,
        buf: &[u8],
        original_file: Option<&OverlaidINode>,
    ) -> Result<usize>;

    /// Poll the events, return a bitmap of events.
    fn poll(&self, original_file: Option<&OverlaidINode>) -> Result<PollStatus>;

    /// Sync data (not include metadata)
    fn sync_data(&self, original_file: Option<&OverlaidINode>) -> Result<()>;

    /// Control device
    fn io_control(
        &self,
        cmd: u32,
        data: usize,
        original_file: Option<&OverlaidINode>,
    ) -> Result<()>;

    /// This is used to implement dynamics cast.
    /// Simply return self in the implement of the function.
    fn as_any_ref(&self) -> &Any;

    /// Create mmap handler for given arguments.
    fn mmap(
        &self,
        start_addr: usize,
        end_addr: usize,
        prot: usize,
        offset: usize,
        original_file: Option<&OverlaidINode>,
    ) -> Result<Box<MemoryHandler>>;

    fn overrideSymbolLink(
        &self,
        original_file: Option<&OverlaidINode>,
    ) -> Option<Arc<INodeContainer>>;
}

pub struct OverlaidINode {
    pub device_file: Arc<INode>,
    pub device_impl: Arc<DeviceHandle>,
}

pub trait INodeExtraOps {
    /// Functions below are used by character device file, or strange non-POSIX files like those under /proc/ (e.g. /proc/self/exec and /proc/114514/mem)
    /// They are implemented by default in a reasonable(?) way.

    /// Create mmap handler for given arguments.
    fn mmap(
        self: &Arc<Self>,
        start_addr: usize,
        end_addr: usize,
        prot: usize,
        offset: usize,
    ) -> Result<Box<MemoryHandler>>;

    /// Hijacking symbolic link. Useful when implementing things like procfs.

    fn overrideSymbolLink(&self) -> Option<Arc<INodeContainer>>;
}

impl INode for OverlaidINode {
    fn read_at(&self, offset: usize, buf: &mut [u8]) -> Result<usize> {
        self.device_impl.read_at(offset, buf, Some(self))
    }

    fn write_at(&self, offset: usize, buf: &[u8]) -> Result<usize> {
        self.device_impl.write_at(offset, buf, Some(self))
    }

    fn poll(&self) -> Result<PollStatus> {
        self.device_impl.poll(Some(self))
    }

    fn metadata(&self) -> Result<Metadata> {
        self.device_file.metadata()
    }

    fn set_metadata(&self, metadata: &Metadata) -> Result<()> {
        self.device_file.set_metadata(metadata)
    }

    fn sync_all(&self) -> Result<()> {
        self.device_impl.sync_data(Some(self))?;
        self.device_file.sync_all()
    }

    fn sync_data(&self) -> Result<()> {
        self.device_impl.sync_data(Some(self))
    }

    fn resize(&self, len: usize) -> Result<()> {
        self.device_file.resize(len)
    }

    fn create(&self, name: &str, type_: FileType, mode: u32) -> Result<Arc<INode>> {
        self.device_file.create(name, type_, mode)
    }

    fn link(&self, name: &str, other: &Arc<INode>) -> Result<()> {
        self.device_file.link(name, other)
    }

    fn unlink(&self, name: &str) -> Result<()> {
        self.device_file.unlink(name)
    }

    fn move_(&self, old_name: &str, target: &Arc<INode>, new_name: &str) -> Result<()> {
        self.device_file.move_(old_name, target, new_name)
    }

    fn find(&self, name: &str) -> Result<Arc<INode>> {
        self.device_file.find(name)
    }

    fn get_entry(&self, id: usize) -> Result<String> {
        self.device_file.get_entry(id)
    }

    fn io_control(&self, cmd: u32, data: usize) -> Result<()> {
        self.device_impl.io_control(cmd, data, Some(self))
    }

    fn fs(&self) -> Arc<FileSystem> {
        self.device_file.fs()
    }

    fn as_any_ref(&self) -> &Any {
        self
    }
}

/// Default impl for a general file INode.
impl INodeExtraOps for INode {
    /// Returning delayed-file mapping.
    /// This is how it should work on most sane filesystems.
    /// This behaviour can be changed to implement strange memory map, like /proc/114514/mem.
    fn mmap(
        self: &Arc<INode>,
        start_addr: usize,
        end_addr: usize,
        prot: usize,
        offset: usize,
    ) -> Result<Box<MemoryHandler>> {
        if let Some(oinode) = self.downcast_ref::<OverlaidINode>() {
            oinode
                .device_impl
                .mmap(start_addr, end_addr, prot, offset, Some(&oinode))
        } else {
            Ok(Box::new(File {
                file: INodeForMap(Arc::clone(self)),
                mem_start: start_addr,
                file_start: offset,
                file_end: offset + end_addr - start_addr,
                allocator: GlobalFrameAlloc,
            }))
        }
    }
    /// No hijack. Use usual symbolic link.
    /// This is how it should work on sane filesystems.
    /// This behaviour can be changed to implement strange symbolic link, like /proc/114514/root and /proc/self/exec.
    fn overrideSymbolLink(&self) -> Option<Arc<INodeContainer>> {
        if let Some(oinode) = self.downcast_ref::<OverlaidINode>() {
            oinode.device_impl.overrideSymbolLink(Some(&oinode))
        } else {
            None
        }
    }
}

pub trait DeviceFileProvider: Send + Sync {
    fn open(&self, minor: usize) -> Option<Arc<DeviceHandle>>;
}

pub struct CDevManager {
    dev_map: BTreeMap<usize, Box<DeviceFileProvider>>,
}
pub static mut CDEV_MANAGER: Option<RwLock<CDevManager>> = None;

pub fn dev_major(dev: u64) -> u32 {
    (dev >> 8) as u32
}
pub fn dev_minor(dev: u64) -> u32 {
    (dev & 0xff) as u32
}

impl CDevManager {
    pub fn findDevice(path: &str, cwd: &PathConfig) -> Result<(Arc<INodeContainer>, usize)> {
        match cwd.path_resolve(&cwd.cwd, path, true)? {
            PathResolveResult::IsFile { file, .. } => {
                let metadata = file.inode.metadata()?;
                if metadata.type_ != FileType::CharDevice {
                    return Err(FsError::InvalidParam);
                }
                Ok((file, metadata.rdev))
            }
            PathResolveResult::IsDir { .. } => Err(FsError::NotFile),
            PathResolveResult::NotExist { .. } => Err(FsError::EntryNotFound),
        }
    }
    pub fn new() -> CDevManager {
        CDevManager {
            dev_map: BTreeMap::new(),
        }
    }
    pub fn init() {
        unsafe {
            CDEV_MANAGER = Some(RwLock::new(CDevManager::new()));
        }
        //let mut cdevm=CDevManager::get().write();
        //crate::arch::board::emmc::register_emmc();
        //cdevm.registerDevice(20, super::hello_device::get_cdev());
    }
    pub fn registerDevice(&mut self, major: usize, deviceprovider: Box<DeviceFileProvider>) {
        info!("Registering device for {}", major);
        self.dev_map.insert(major, deviceprovider);
    }
    /// This can be used by kernel to open a handle unrelated to the file.
    pub fn openDeviceHandle(&self, major: usize, minor: usize) -> Option<Arc<DeviceHandle>> {
        let provider = self.dev_map.get(&major)?;
        provider.open(minor)
    }
    /// This can be used to decorate the INode.
    pub fn openINode(
        &self,
        inode: &Arc<INode>,
        major: usize,
        minor: usize,
    ) -> Option<OverlaidINode> {
        let device_handle = self.openDeviceHandle(major, minor)?;
        Some(OverlaidINode {
            device_file: Arc::clone(inode),
            device_impl: device_handle,
        })
    }
    pub fn get() -> &'static RwLock<CDevManager> {
        unsafe { CDEV_MANAGER.as_ref().unwrap() }
    }
}

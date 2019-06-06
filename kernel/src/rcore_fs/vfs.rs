use alloc::{string::String, sync::Arc, vec::Vec};
use core::any::Any;
use core::fmt;
use core::result;
use core::str;

/// Abstract operations on a inode.
pub trait INode: Any + Sync + Send {
    /// Read bytes at `offset` into `buf`, return the number of bytes read.
    fn read_at(&self, offset: usize, buf: &mut [u8]) -> Result<usize>;

    /// Write bytes at `offset` from `buf`, return the number of bytes written.
    fn write_at(&self, offset: usize, buf: &[u8]) -> Result<usize>;

    /// Poll the events, return a bitmap of events.
    fn poll(&self) -> Result<PollStatus>;

    /// Get metadata of the INode
    fn metadata(&self) -> Result<Metadata>;

    /// Set metadata of the INode
    fn set_metadata(&self, metadata: &Metadata) -> Result<()>;

    /// Sync all data and metadata
    fn sync_all(&self) -> Result<()>;

    /// Sync data (not include metadata)
    fn sync_data(&self) -> Result<()>;

    /// Resize the file
    fn resize(&self, len: usize) -> Result<()>;

    /// Create a new INode in the directory
    fn create(&self, name: &str, type_: FileType, mode: u32)->Result<Arc<INode>>{
        self.create2(name, type_, mode, 0)
    }
    fn create2(&self, name: &str, type_: FileType, mode: u32,  rdev: usize) -> Result<Arc<INode>>{
        self.create(name, type_, mode)
    }

    /// Create a hard link `name` to `other`
    fn link(&self, name: &str, other: &Arc<INode>) -> Result<()>;

    /// Delete a hard link `name`
    fn unlink(&self, name: &str) -> Result<()>;

    /// Move INode `self/old_name` to `target/new_name`.
    /// If `target` equals `self`, do rename.
    fn move_(&self, old_name: &str, target: &Arc<INode>, new_name: &str) -> Result<()>;

    /// Find the INode `name` in the directory
    fn find(&self, name: &str) -> Result<Arc<INode>>;

    /// Get the name of directory entry
    fn get_entry(&self, id: usize) -> Result<String>;

    /// Control device
    fn io_control(&self, cmd: u32, data: usize) -> Result<()>;

    /// Get the file system of the INode
    fn fs(&self) -> Arc<FileSystem>;

    /// This is used to implement dynamics cast.
    /// Simply return self in the implement of the function.
    fn as_any_ref(&self) -> &Any;

    // Stub implementation for setting an rdev.
    // This is ensured to be called after create.
    fn setrdev(&self, dev: u64) -> Result<()> {
        Ok(())
    }
}

impl INode {
    pub fn downcast_ref<T: INode>(&self) -> Option<&T> {
        self.as_any_ref().downcast_ref::<T>()
    }
    pub fn list(&self) -> Result<Vec<String>> {
        let info = self.metadata()?;
        if info.type_ != FileType::Dir {
            return Err(FsError::NotDir);
        }
        (0..info.size).map(|i| self.get_entry(i)).collect()
    }
}
#[derive(Debug, Default)]
pub struct PollStatus {
    pub read: bool,
    pub write: bool,
    pub error: bool,
}
/// Metadata of INode
///
/// Ref: [http://pubs.opengroup.org/onlinepubs/009604499/basedefs/sys/stat.h.html]
#[derive(Debug, Eq, PartialEq)]
pub struct Metadata {
    /// Device ID
    pub dev: usize,
    /// Inode number
    pub inode: usize,
    /// Size in bytes
    ///
    /// SFS Note: for normal file size is the actuate file size
    /// for directory this is count of dirent.
    pub size: usize,
    /// A file system-specific preferred I/O block size for this object.
    /// In some file system types, this may vary from file to file.
    pub blk_size: usize,
    /// Size in blocks
    pub blocks: usize,
    /// Time of last access
    pub atime: Timespec,
    /// Time of last modification
    pub mtime: Timespec,
    /// Time of last change
    pub ctime: Timespec,
    /// Type of file
    pub type_: FileType,
    /// Permission
    pub mode: u16,
    /// Number of hard links
    ///
    /// SFS Note: different from linux, "." and ".." count in nlinks
    /// this is same as original ucore.
    pub nlinks: usize,
    /// User ID
    pub uid: usize,
    /// Group ID
    pub gid: usize,
    // Currently we use two u32 to store the rdev on sfs.
    pub rdev: u64,
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Hash)]
pub struct Timespec {
    pub sec: i64,
    pub nsec: i32,
}

#[derive(Debug, Eq, PartialEq, Copy, Clone)]
pub enum FileType {
    File,
    Dir,
    SymLink,
    CharDevice,
    BlockDevice,
    NamedPipe,
    Socket,
}

/// Metadata of FileSystem
///
/// Ref: [http://pubs.opengroup.org/onlinepubs/9699919799/]
#[derive(Debug)]
pub struct FsInfo {
    /// File system block size
    pub bsize: usize,
    /// Fundamental file system block size
    pub frsize: usize,
    /// Total number of blocks on file system in units of `frsize`
    pub blocks: usize,
    /// Total number of free blocks
    pub bfree: usize,
    /// Number of free blocks available to non-privileged process
    pub bavail: usize,
    /// Total number of file serial numbers
    pub files: usize,
    /// Total number of free file serial numbers
    pub ffree: usize,
    /// Maximum filename length
    pub namemax: usize,
}

// Note: IOError/NoMemory always lead to a panic since it's hard to recover from it.
//       We also panic when we can not parse the fs on disk normally
#[derive(Debug)]
pub enum FsError {
    NotSupported,  //E_UNIMP, or E_INVAL
    NotFile,       //E_ISDIR
    IsDir,         //E_ISDIR, used only in link
    NotDir,        //E_NOTDIR
    EntryNotFound, //E_NOENT
    EntryExist,    //E_EXIST
    NotSameFs,     //E_XDEV
    InvalidParam,  //E_INVAL
    NoDeviceSpace, //E_NOSPC, but is defined and not used in the original ucore, which uses E_NO_MEM
    DirRemoved,    //E_NOENT, when the current dir was remove by a previous unlink
    DirNotEmpty,   //E_NOTEMPTY
    WrongFs,       //E_INVAL, when we find the content on disk is wrong when opening the device
    DeviceError,
    SymLoop,  //E_LOOP, too many symlink follows.
    NoDevice, //E_NXIO
    IOCTLError,
    Again, // E_AGAIN, when no data is available, never happens in fs
}

impl fmt::Display for FsError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

#[cfg(any(test, feature = "std"))]
impl std::error::Error for FsError {}

pub type Result<T> = result::Result<T, FsError>;

/// Abstract filesystem
pub trait FileSystem: Sync + Send {
    fn sync(&self) -> Result<()>;
    fn root_inode(&self) -> Arc<INode>;
    fn info(&self) -> FsInfo;
    // TODO: good ways to force attribute? For example, a filesystem must contain an owner attribute pointing to a module.
}

use alloc::borrow::ToOwned;
use alloc::boxed::Box;
use alloc::collections::btree_map::*;
use alloc::sync::Weak;
use core::sync::*;
use spin::RwLock;

use crate::drivers::BlockDriver;
use crate::rcore_fs_sfs::SimpleFileSystem;
use alloc::slice::*;
use core::mem::*;
use crate::rcore_fs::dev::WriteThroughRandomBlockDevice;

type INodeId = usize;

/// The filesystem on which all the other filesystems are mounted
pub struct RootFS {
    pub filesystem: Arc<FileSystem>,
    pub mountpoints: BTreeMap<INodeId, Arc<RwLock<RootFS>>>,
    pub self_mountpoint: Arc<INodeContainer>,
    pub self_ref: Weak<RwLock<RootFS>>,
}

#[derive(Clone)]
pub struct INodeContainer {
    pub inode: Arc<INode>, //TODO: deref to this
    pub vfs: Arc<RwLock<RootFS>>,
}

impl RootFS {
    pub fn init() -> Arc<RwLock<Self>> {
        RootFS {
            filesystem: RootFS::init_mount_sfs(),
            mountpoints: BTreeMap::new(),
            self_mountpoint: Arc::new(unsafe { uninitialized() }), // This should NOT EVER BE ACCESSED!
            self_ref: Weak::default(),
        }
        .wrap()
    }

    /// Wrap pure `RootFS` with `Arc<RwLock<..>>`.
    /// Used in constructors.
    pub fn wrap(self) -> Arc<RwLock<Self>> {
        // Create an Arc, make a Weak from it, then put it into the struct.
        // It's a little tricky.
        info!("wrap");
        let fs = Arc::new(RwLock::new(self));
        let weak = Arc::downgrade(&fs);
        let ptr = Arc::into_raw(fs) as *mut RwLock<Self>;
        unsafe {
            (*ptr).write().self_ref = weak;
            info!("wrapped");
            Arc::from_raw(ptr)
        }
    }

    // TODO: mount sfs onto root.
    // This is somehow hard work to do: since you may want to unify the process.
    // 1. Boot from a filesystem like initramfs, which can be a readonly SFS mounted onto root.
    //    This means you can bundle kernel modules into kernel by packaging them in initramfs.
    // 2. Mount /dev and place /dev/sda (while naming /dev/sda itself is a hard problem that is related with universal device management).
    // 3. Remount root, replacing initramfs with /dev/sda (this requires connecting filesystem to device system).
    //    A hacky approach to avoid implementing re-mounting is to mount /dev/sda under initramfs and perform a chroot.
    //    But in this way you must simulate chroot-jailbreaking behaviour properly: even if some application breaks the jail, it should not ever touch initramfs, or you're caught cheating.
    //    Or... you can swap the SFS with VIRTUAL_FS?

    fn init_mount_sfs() -> Arc<FileSystem> {
        #[cfg(not(feature = "link_user"))]
        let device = {
            #[cfg(any(target_a = "riscv32", target_arch = "riscv64", target_arch = "x86_64"))]
            {
                Arc::new(WriteThroughRandomBlockDevice(Arc::new(BlockDriver(
                    crate::drivers::BLK_DRIVERS
                        .read()
                        .iter()
                        .next()
                        .expect("Block device not found")
                        .clone(),
                ))))
            }
            #[cfg(target_arch = "aarch64")]
            {
                unimplemented!()
            }
        };
        #[cfg(feature = "link_user")]
        let device = {
            extern "C" {
                fn _user_img_start();
                fn _user_img_end();
            }
            info!(
                "SFS linked to kernel, from {:08x} to {:08x}",
                _user_img_start as usize, _user_img_end as usize
            );
            use crate::fs::device::MemBuf;
            let result=Arc::new(unsafe { MemBuf::new(_user_img_start, _user_img_end) });
            info!("Done");
            result
        };

        let sfs=SimpleFileSystem::open(device).expect("failed to open SFS");
        info!("fs");
        sfs
    }

    pub fn root_inode(&self) -> Arc<INodeContainer> {
        Arc::new(INodeContainer {
            inode: self.filesystem.root_inode(),
            vfs: self.self_ref.upgrade().unwrap(),
        })
    }

    pub fn overlaid_mount_point(&self, ic: Arc<INodeContainer>) -> Arc<INodeContainer> {
        let inode_id = ic.inode.metadata().unwrap().inode;
        if let Some(sub_vfs) = self.mountpoints.get(&inode_id) {
            let mut sub_inode = sub_vfs.read().root_inode();
            //sub_inode.original_mountpoint=Some(Arc::new(ic));
            sub_inode
        } else {
            ic
        }
    }
}

#[derive(Clone)]
pub struct PathConfig {
    pub root: Arc<INodeContainer>, // ensured to be a dir.
    pub cwd: Arc<INodeContainer>,  // ensured to be a dir.
}

/// The enum used to represent result of a successful path resolve.
pub enum PathResolveResult {
    IsDir {
        // You can always get the parent directory by inode, so no necessity to take with parent.
        dir: Arc<INodeContainer>,
    },
    IsFile {
        // If it is a file, its parent must have been touched.
        // This is also returned for further symbol resolving, since resolving symbol needs parent directory.
        file: Arc<INodeContainer>,
        parent: Arc<INodeContainer>,
        name: String,
    },
    NotExist {
        // If it is not found, its parent must have been touched. This is useful when dealing with syscalls like creat or rename.
        parent: Arc<INodeContainer>,
        name: String,
    },
}

// Path resolution must be done with a root.
// A better name is "Filesystem Selector", like the "segment selector".
impl PathConfig {
    pub fn init_root() -> PathConfig {
        let root = super::get_virtual_fs().read().root_inode();
        let cwd = root.clone();
        PathConfig { root, cwd }
    }

    pub fn path_resolve(
        &self,
        cwd: &Arc<INodeContainer>,
        path: &str,
        resolve_last_symbol: bool,
    ) -> Result<PathResolveResult> {
        let mut follow_counter = 40;
        let depth_counter = 10;
        let r = self.resolve_path(cwd, path, &mut follow_counter, depth_counter)?;
        if resolve_last_symbol {
            if let PathResolveResult::IsFile { file, parent, .. } = r {
                return self.resolve_symbol_recursively(&parent, &file, &mut follow_counter, depth_counter);
            }
        }
        Ok(r)
    }

    pub fn resolve_parent(&self, cwd: &Arc<INodeContainer>) -> Arc<INodeContainer> {
        cwd.find(self.has_reached_root(&cwd), "..").unwrap() // There is no reason that this can fail, as long as cwd is really a directory.
    }

    /// This call is used by getcwd() to detect possible leaks.
    /// All files are organized in a big tree, so it will eventually achieve the root.
    pub unsafe fn force_resolve_parent(&self, cwd: &Arc<INodeContainer>) -> Arc<INodeContainer> {
        cwd.find(false, "..").unwrap()
    }

    pub fn resolve_path(
        &self,
        cwd: &Arc<INodeContainer>,
        path: &str,
        follow_counter: &mut usize,
        depth_counter: usize,
    ) -> Result<PathResolveResult> {
        debug!("Path resolution {}", path);
        let mut cwd = Arc::clone({
            if path.starts_with("/") {
                &self.root
            } else {
                if cwd.inode.metadata().unwrap().type_ != FileType::Dir {
                    return Err(FsError::NotDir);
                }
                cwd //must be a dir, or an error will be thrown.
            }
        });
        let parts: Vec<&str> = path.split("/").collect();
        let (last_part, mid_part) = parts.split_last().unwrap();

        for part in mid_part.iter() {
            if *part == "" {
                continue;
            }
            debug!("Resolve part: {}", part);
            let next = cwd.find(self.has_reached_root(&cwd), part)?;
            debug!("solve link");
            // Try solve symbolic link.
            let symlink_solve_result =
                self.resolve_symbol_recursively(&cwd, &next, follow_counter, depth_counter)?;
            match symlink_solve_result {
                PathResolveResult::IsDir { dir } => {
                    cwd = dir;
                }
                PathResolveResult::IsFile { .. } => {
                    return Err(FsError::NotDir);
                }
                PathResolveResult::NotExist { .. } => {
                    return Err(FsError::EntryNotFound);
                }
            }
        }
        debug!("Last part {}", last_part);
        // Resolving last part.
        let next = cwd.find(self.has_reached_root(&cwd), last_part);
        debug!("match next");
        match next {
            Ok(next) => {
                debug!("Ok!");
                //No extra check needed, since extra work can be done through check.
                if next.inode.metadata().unwrap().type_ == FileType::Dir {
                    Ok(PathResolveResult::IsDir { dir: next })
                } else {
                    Ok(PathResolveResult::IsFile {
                        parent: cwd,
                        file: next,
                        name: String::from(*last_part),
                    })
                }
            }
            Err(FsError::EntryNotFound) => Ok(PathResolveResult::NotExist {
                name: String::from(*last_part),
                parent: cwd,
            }),
            Err(x) => Err(x),
        }
    }

    /// Resolves symbol by one layer.
    ///
    /// TODO:
    ///   Linux proc fs has some anti-POSIX magics here, like /proc/[pid]/root.
    ///   In those cases, those magics points to strange places, without following symlink rules.
    ///   This hack can be achieved here.
    pub fn resolve_symbol(
        &self,
        cwd: &Arc<INodeContainer>,
        symbol: &Arc<INodeContainer>,
        follow_counter: &mut usize,
        depth_counter: usize,
    ) -> Result<PathResolveResult> {
        if depth_counter == 0 {
            return Err(FsError::SymLoop);
        }
        if *follow_counter > 0 {
            *follow_counter -= 1;
            let mut content = [0u8; 256];
            let len = symbol.inode.read_at(0, &mut content)?;
            if let Ok(path) = str::from_utf8(&content[..len]) {
                self.resolve_path(cwd, path, follow_counter, depth_counter - 1)
            } else {
                return Err(FsError::NotDir);
            }
        } else {
            Err(FsError::SymLoop)
        }
    }
    /// Resolves symbol recursively.
    /// Note that a not-found will cause the resolved symbol pointing to the final file.
    pub fn resolve_symbol_recursively(
        &self,
        cwd: &Arc<INodeContainer>,
        symbol: &Arc<INodeContainer>,
        follow_counter: &mut usize,
        depth_counter: usize,
    ) -> Result<PathResolveResult> {
        let mut current_symbol_dir = Arc::clone(cwd);
        let mut current_symbol = Arc::clone(symbol);
        let mut current_name = String::new();
        while current_symbol.inode.metadata().unwrap().type_ == FileType::SymLink {
            let resolve_result = self.resolve_symbol(
                &current_symbol_dir,
                &current_symbol,
                follow_counter,
                depth_counter,
            )?;
            match resolve_result {
                PathResolveResult::NotExist { .. } => {
                    return Ok(resolve_result);
                }
                PathResolveResult::IsDir { .. } => {
                    return Ok(resolve_result);
                }
                PathResolveResult::IsFile { file, parent, name } => {
                    current_symbol = file;
                    current_symbol_dir = parent;
                    current_name = name;
                }
            }
        }
        if current_symbol.inode.metadata().unwrap().type_ == FileType::Dir {
            Ok(PathResolveResult::IsDir {
                dir: current_symbol,
            })
        } else {
            Ok(PathResolveResult::IsFile {
                file: current_symbol,
                parent: current_symbol_dir,
                name: current_name,
            })
        }
    }
    pub fn has_reached_root(&self, current: &INodeContainer) -> bool {
        Arc::ptr_eq(&current.vfs, &self.root.vfs)
            && self.root.inode.metadata().unwrap().inode == current.inode.metadata().unwrap().inode
    }
}

// XXX: what's the meaning?
      // The unsafe filesystem for Stdin, Stdout, anonymous pipe and so on.
      // If you don't touch it you will not break it.
      // But in fact you should detect file operations (e.g. fstat) on virtual files and prevent them.
pub static mut ANONYMOUS_FS: Option<Arc<RwLock<RootFS>>> = None;

pub fn get_anonymous_fs() -> &'static Arc<RwLock<RootFS>> {
    unsafe { ANONYMOUS_FS.as_ref().unwrap() }
}

impl INodeContainer {
    pub fn is_very_root(&self) -> bool {
        PathConfig::init_root().has_reached_root(self)
    }
    pub fn is_root_inode(&self) -> bool {
        self.inode.fs().root_inode().metadata().unwrap().inode
            == self.inode.metadata().unwrap().inode
    }

    /// Creates an anonymous inode.
    /// Should not be used as a location at any time, or be totally released at any time.
    pub unsafe fn anonymous_inode(inode: Arc<INode>) -> Arc<INodeContainer> {
        Arc::new(INodeContainer {
            inode,
            vfs: Arc::clone(get_anonymous_fs()),
        })
    }

    /// Does a one-level finding.
    pub fn find(self: &Arc<INodeContainer>, root: bool, next: &str) -> Result<Arc<INodeContainer>> {
        debug!("finding name {}", next);
        debug!("in {:?}", self.inode.list().unwrap());
        match next {
            "" | "." => Ok(Arc::clone(&self)),
            ".." => {
                // Going Up
                // We need to check these things:
                // 1. Is going forward allowed, considering the current root?
                // 2. Is going forward trespassing the filesystem border,
                //    thus requires falling back to parent of original_mountpoint?
                // TODO: check going up.
                if root {
                    Ok(Arc::clone(&self))
                } else if self.is_root_inode() {
                    // Here is mountpoint.
                    self.vfs.read().self_mountpoint.find(root, "..")
                } else {
                    // Not trespassing filesystem border. Parent and myself in the same filesystem.
                    Ok(Arc::new(INodeContainer {
                        inode: self.inode.find(next)?, // Going up is handled by the filesystem. A better API?
                        vfs: Arc::clone(&self.vfs),
                    }))
                }
            }
            _ => {
                // Going down may trespass the filesystem border.
                // An INode replacement is required here.
                let next_ic = Arc::new(INodeContainer {
                    inode: self.inode.find(next)?,
                    vfs: Arc::clone(&self.vfs),
                });
                debug!("find Ok!");
                Ok(self.vfs.read().overlaid_mount_point(next_ic))
            }
        }
    }


    /// If `child` is a child of `self`, return its name.
    pub fn find_name_by_child(
        self: &Arc<INodeContainer>,
        child: &Arc<INodeContainer>,
    ) -> Result<String> {
        for index in 0.. {
            let name = self.inode.get_entry(index)?;
            match name.as_ref() {
                "." | ".." => {}
                _ => {
                    let queryback = self.find(false, &name)?;
                    let queryback = self.vfs.read().overlaid_mount_point(queryback);
                    // TODO: mountpoint check!
                    debug!("checking name {}", name);
                    if Arc::ptr_eq(&queryback.vfs, &child.vfs)
                        && queryback.inode.metadata()?.inode == child.inode.metadata()?.inode
                    {
                        return Ok(name);
                    }
                }
            }
        }
        Err(FsError::EntryNotFound)
    }
}

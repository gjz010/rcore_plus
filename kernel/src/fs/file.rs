//! File handle for process

use alloc::{string::String, sync::Arc};
use rcore_fs_sfs::*;
use rcore_fs::vfs;
use rcore_fs::vfs::{FsError, INode, Metadata};

#[derive(Clone)]
pub struct FileHandle {
    inode: Arc<INode>,
    offset: u64,
    options: OpenOptions,
}

#[derive(Debug, Clone)]
pub struct OpenOptions {
    pub read: bool,
    pub write: bool,
    /// Before each write, the file offset is positioned at the end of the file.
    pub append: bool,
}

#[derive(Debug)]
pub enum SeekFrom {
    Start(u64),
    End(i64),
    Current(i64),
}

impl FileHandle {
    pub fn new(inode: Arc<INode>, options: OpenOptions) -> Self {
        FileHandle {
            inode,
            offset: 0,
            options,
        }
    }

    pub fn read(&mut self, buf: &mut [u8]) -> vfs::Result<usize> {
        let len = self.read_at(self.offset as usize, buf)?;
        self.offset += len as u64;
        Ok(len)
    }

    pub fn read_at(&mut self, offset: usize, buf: &mut [u8]) -> vfs::Result<usize> {
        if !self.options.read {
            return Err(FsError::InvalidParam); // FIXME: => EBADF
        }
        let len = self.inode.read_at(offset, buf)?;
        Ok(len)
    }

    pub fn write(&mut self, buf: &[u8]) -> vfs::Result<usize> {
        let offset = match self.options.append {
            true => self.inode.metadata()?.size as u64,
            false => self.offset,
        } as usize;
        let len = self.write_at(offset, buf)?;
        self.offset = (offset + len) as u64;
        Ok(len)
    }

    pub fn write_at(&mut self, offset: usize, buf: &[u8]) -> vfs::Result<usize> {
        if !self.options.write {
            return Err(FsError::InvalidParam); // FIXME: => EBADF
        }
        let len = self.inode.write_at(offset, buf)?;
        Ok(len)
    }

    pub fn seek(&mut self, pos: SeekFrom) -> vfs::Result<u64> {
        self.offset = match pos {
            SeekFrom::Start(offset) => offset,
            SeekFrom::End(offset) => (self.inode.metadata()?.size as i64 + offset) as u64,
            SeekFrom::Current(offset) => (self.offset as i64 + offset) as u64,
        };
        Ok(self.offset)
    }

    pub fn set_len(&mut self, len: u64) -> vfs::Result<()> {
        if !self.options.write {
            return Err(FsError::InvalidParam); // FIXME: => EBADF
        }
        self.inode.resize(len as usize)?;
        Ok(())
    }

    pub fn sync_all(&mut self) -> vfs::Result<()> {
        self.inode.sync_all()
    }

    pub fn sync_data(&mut self) -> vfs::Result<()> {
        self.inode.sync_data()
    }

    pub fn metadata(&self) -> vfs::Result<Metadata> {
        self.inode.metadata()
    }

    pub fn lookup_follow(&self, path: &str, max_follow: usize) -> vfs::Result<Arc<INode>> {
        self.inode.lookup_follow(path, max_follow)
    }

    pub fn read_entry(&mut self) -> vfs::Result<String> {
        if !self.options.read {
            return Err(FsError::InvalidParam); // FIXME: => EBADF
        }
        let name = self.inode.get_entry(self.offset as usize)?;
        self.offset += 1;
        Ok(name)
    }

    pub fn call_ioctl(&self, request: u32, data: *mut u8) -> Result<(), IOCTLError> {
        let inode_impl = self.inode.downcast_ref::<INodeImpl>().unwrap();
        inode_impl.call_ioctl(request, data)
    }

}

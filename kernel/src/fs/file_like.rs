use core::fmt;

use rcore_fs::vfs;
use rcore_fs_sfs::*;
use super::FileHandle;

use crate::net::Socket;
use crate::syscall::{SysResult, SysError};
use alloc::boxed::Box;

// TODO: merge FileLike to FileHandle ?
// TODO: fix dup and remove Clone
#[derive(Clone)]
pub enum FileLike {
    File(FileHandle),
    Socket(Box<dyn Socket>),
}

impl FileLike {
    pub fn read(&mut self, buf: &mut [u8]) -> SysResult {
        let len = match self {
            FileLike::File(file) => file.read(buf)?,
            FileLike::Socket(socket) => socket.read(buf).0?,
        };
        Ok(len)
    }
    pub fn write(&mut self, buf: &[u8]) -> SysResult {
        let len = match self {
            FileLike::File(file) => file.write(buf)?,
            FileLike::Socket(socket) => socket.write(buf, None)?,
        };
        Ok(len)
    }
    pub fn call_ioctl(&mut self, request: u32, data: *mut u8) -> SysResult {
        match self {
            FileLike::File(file) =>
                match file.call_ioctl(request, data) {
                    Ok(x) => Ok(0),
                    Err(x) => Err(match x {
                        IOCTLError::NotValidFD => SysError::EBADF,
                        IOCTLError::NotValidMemory => SysError::EFAULT,
                        IOCTLError::NotValidParam => SysError::EINVAL,
                        IOCTLError::NotCharDevice => SysError::ENOTTY
                    })
                },
            FileLike::Socket(socket) => Err(SysError::ENOTTY)
        }
    }
}

impl fmt::Debug for FileLike {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            FileLike::File(_) => write!(f, "File"),
            FileLike::Socket(_) => write!(f, "Socket"),
        }
    }
}

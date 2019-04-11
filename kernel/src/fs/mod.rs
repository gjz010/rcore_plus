use alloc::{sync::Arc, vec::Vec};

use rcore_fs::vfs::*;
use rcore_fs_sfs::SimpleFileSystem;
use rcore_fs_sfs::INodeImpl;

#[cfg(target_arch = "x86_64")]
use crate::arch::driver::ide;

pub use self::file::*;
pub use self::file_like::*;
pub use self::pipe::Pipe;
pub use self::stdio::{STDIN, STDOUT, AUDIO};

mod device;
mod file;
mod file_like;
mod pipe;
mod stdio;

/// Hard link user programs
#[cfg(feature = "link_user")]
global_asm!(concat!(
    r#"
	.section .data
	.global _user_img_start
	.global _user_img_end
        .global _blank_img_start
        .global _blank_img_end
_user_img_start:
    .incbin ""#,
    env!("SFSIMG"),
    r#""
_user_img_end:
_blank_img_start:
    .incbin ""#,
    env!("BLANKIMG"),
    r#""
_blank_img_end:
"#
));

lazy_static! {
    /// The root of file system
    // pub static ref DEV;

    pub static ref SFS: Arc<SimpleFileSystem> = {
           #[cfg(not(feature = "link_user"))]
        let device = {
            #[cfg(any(target_arch = "riscv32", target_arch = "riscv64"))]
            {
                crate::drivers::BLK_DRIVERS.read().iter()
                    .next().expect("VirtIOBlk not found")
                    .clone()
            }
            #[cfg(target_arch = "x86_64")]
            {
                Arc::new(ide::IDE::new(1))
            }
        };
        #[cfg(feature = "link_user")]
        let device = {
            extern {
                fn _user_img_start();
                fn _user_img_end();
                fn _blank_img_start();
                fn _blank_img_end();
            }
            // Arc::new(unsafe { device::MemBuf::new(_user_img_start, _user_img_end) })
            Arc::new(unsafe { device::MemBuf::new(_user_img_start, _blank_img_end) })
        };

        let sfs = SimpleFileSystem::open(device).expect("failed to open SFS");
        sfs
    };

    pub static ref ROOT_INODE: Arc<INode> = {
        let root = SFS.root_inode();
        root
    };
}

pub const FOLLOW_MAX_DEPTH: usize = 1;

pub trait INodeExt {
    fn read_as_vec(&self) -> Result<Vec<u8>>;
}

impl INodeExt for INode {
    fn read_as_vec(&self) -> Result<Vec<u8>> {
        let size = self.metadata()?.size;
        let mut buf = Vec::with_capacity(size);
        unsafe {
            buf.set_len(size);
        }
        self.read_at(0, buf.as_mut_slice())?;
        Ok(buf)
    }
}

pub fn init() {
    let dev_inode = ROOT_INODE.create("dev", FileType::Dir, 0).expect("fail to create dev");
}

//! Entrance and initialization for aarch64.

pub mod consts;
pub mod cpu;
pub mod driver;
pub mod interrupt;
pub mod io;
pub mod memory;
pub mod paging;
pub mod rand;
pub mod syscall;
pub mod timer;
pub mod ipi;
#[cfg(feature = "board_raspi3")]
#[path = "board/raspi3/mod.rs"]
pub mod board;

global_asm!(include_str!("boot/entry.S"));

/// The entry point of kernel
#[no_mangle] // don't mangle the name of this function
pub extern "C" fn rust_main() -> ! {
    board::init_serial_early();

    crate::logging::init();
    interrupt::init();
    memory::init();
    // We first load kernel-module managers...
    // So that we can plug drivers in.
    // Startup ModuleManager.
    crate::lkm::manager::ModuleManager::init();

    // Startup CDevManager
    crate::lkm::cdev::CDevManager::init();

    driver::init();
    // Startup FileSystemManager
    crate::lkm::fs::FileSystemManager::init();
    info!("init fs");
    crate::rcore_fs::init();
    info!("fs done");
    println!("{}", LOGO);

    crate::process::init();

    crate::kmain();
}

const LOGO: &str = r#"
    ____                __   ____  _____
   / __ \ __  __ _____ / /_ / __ \/ ___/
  / /_/ // / / // ___// __// / / /\__ \
 / _, _// /_/ /(__  )/ /_ / /_/ /___/ /
/_/ |_| \__,_//____/ \__/ \____//____/
"#;

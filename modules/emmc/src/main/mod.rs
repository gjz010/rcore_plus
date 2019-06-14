extern crate alloc;
extern crate rcore;

pub mod emmc;
#[no_mangle]
pub extern "C" fn init_module() {
    emmc::EmmcDriver::init();
}

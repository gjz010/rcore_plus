extern crate alloc;
extern crate rcore;

pub mod audio;
#[no_mangle]
pub extern "C" fn init_module() {
    audio::AudioDriver::init();
}

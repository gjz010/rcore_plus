#![no_std]
extern crate rcore;

use rcore::lkm::ffi;
pub mod hello;

#[no_mangle]
pub extern "C" fn init_module(){
    rcore::lkm::api::lkm_api_pong();
    ffi::patch_isize_to_usize(10);
    hello::hello_again();
}


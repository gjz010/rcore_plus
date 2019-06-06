// Copied from musl ldso.
pub mod x86_64;
pub mod aarch64;

#[cfg(target_arch = "x86_64")]
pub use super::loader::x86_64::*;

#[cfg(target_arch = "aarch64")]
pub use super::loader::aarch64::*;
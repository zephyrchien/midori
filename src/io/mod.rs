mod copy;
pub use copy::copy;

#[cfg(target_os = "linux")]
mod zero_copy;
#[cfg(target_os = "linux")]
pub use zero_copy::zero_copy;

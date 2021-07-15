use std::io;

mod types;
mod consts;
pub use types::CommonAddr;
pub use consts::BUF_SIZE;
#[cfg(target_os = "linux")]
pub use consts::PIPE_BUF_SIZE;

pub fn new_io_err(e: &str) -> io::Error {
    io::Error::new(io::ErrorKind::Other, e)
}

pub unsafe fn const_cast<T>(x: &T) -> &mut T {
    let const_ptr = x as *const T;
    let mut_ptr = const_ptr as *mut T;
    &mut *mut_ptr
}

use std::io;
use std::net::{SocketAddr, IpAddr, Ipv4Addr};

mod types;
mod consts;
mod cert;
pub use consts::*;
pub use types::CommonAddr;
pub use cert::{load_certs, load_keys, generate_cert_key};
#[cfg(target_os = "linux")]
pub use consts::PIPE_BUF_SIZE;

#[inline]
pub fn new_io_err(e: &str) -> io::Error {
    io::Error::new(io::ErrorKind::Other, e)
}

#[allow(clippy::mut_from_ref)]
#[inline]
pub unsafe fn const_cast<T>(x: &T) -> &mut T {
    let const_ptr = x as *const T;
    let mut_ptr = const_ptr as *mut T;
    &mut *mut_ptr
}

#[allow(dead_code)]
#[inline]
pub fn empty_sockaddr() -> SocketAddr {
    SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), 0)
}

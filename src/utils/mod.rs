use std::net::{SocketAddr, IpAddr, Ipv4Addr, Ipv6Addr};

pub mod types;
pub mod consts;
pub mod cert;
#[macro_use]
pub mod macros;

pub use consts::*;
pub use types::{CommonAddr, MaybeQuic};
pub use cert::{load_certs, load_keys, generate_cert_key};
pub use must;

#[cfg(target_os = "linux")]
pub use consts::PIPE_BUF_SIZE;

#[allow(clippy::mut_from_ref)]
#[inline]
pub unsafe fn const_cast<T>(x: &T) -> &mut T {
    let const_ptr = x as *const T;
    let mut_ptr = const_ptr as *mut T;
    &mut *mut_ptr
}

#[allow(dead_code)]
#[inline]
pub fn empty_sockaddr_v4() -> SocketAddr {
    SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), 0)
}

#[allow(dead_code)]
#[inline]
pub fn empty_sockaddr_v6() -> SocketAddr {
    SocketAddr::new(IpAddr::V6(Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 0)), 0)
}

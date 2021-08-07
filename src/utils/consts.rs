pub const VERSION: &str = "0.5.1";
pub const NAV_VERSION: &str = "0.1.0";

pub const BUF_SIZE: usize = 0x4000;
pub const NOT_A_DNS_NAME: &str = "localhost";

#[cfg(feature = "ws")]
pub const WS_BUF_SIZE: usize = 0x1000;
#[cfg(feature = "h2c")]
pub const H2_BUF_SIZE: usize = 0x1000;
#[cfg(feature = "udp")]
pub const UDP_BUF_SIZE: usize = 0x1000;
#[cfg(feature = "tls")]
pub const OCSP_BUF_SIZE: usize = 0x400;
#[cfg(target_os = "linux")]
pub const PIPE_BUF_SIZE: usize = 0x10000;

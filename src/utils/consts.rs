use lazy_static::lazy_static;
use rustls_native_certs;
use rustls::RootCertStore;

pub const BUF_SIZE: usize = 0x4000;
pub const PIPE_BUF_SIZE: usize = 0x10000;
pub const OCSP_BUF_SIZE: usize = 0x400;

lazy_static! {
    pub static ref NATIVE_CERTS: RootCertStore =
        rustls_native_certs::load_native_certs().unwrap();
}

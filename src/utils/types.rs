use std::fmt::{self, Display, Formatter};
use std::net::SocketAddr;

#[cfg(unix)]
use std::path::PathBuf;

#[derive(Clone)]
pub enum CommonAddr {
    SocketAddr(SocketAddr),
    DomainName(String, u16),
    #[cfg(unix)]
    UnixSocketPath(PathBuf),
}

impl Display for CommonAddr {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::SocketAddr(sockaddr) => write!(f, "{}", sockaddr),
            Self::DomainName(addr, port) => write!(f, "{}:{}", addr, port),
            #[cfg(unix)]
            Self::UnixSocketPath(path) => write!(f, "{}", path.display()),
        }
    }
}

impl CommonAddr {
    pub fn to_dns_name(&self) -> String {
        match self {
            CommonAddr::DomainName(addr, _) => addr.clone(),
            _ => String::new(),
        }
    }
}

use crate::transport::quic;
pub enum MaybeQuic<L> {
    Quic(quic::RawAcceptor),
    Other(L),
}

impl<L> MaybeQuic<L> {
    pub fn take_quic(self) -> Option<quic::RawAcceptor> {
        match self {
            Self::Quic(x) => Some(x),
            _ => None,
        }
    }
    pub fn take_other(self) -> Option<L> {
        match self {
            Self::Other(x) => Some(x),
            _ => None,
        }
    }
}

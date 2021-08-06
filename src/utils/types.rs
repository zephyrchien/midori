use std::fmt::{self, Display, Formatter};
use std::net::SocketAddr;

#[cfg(all(unix, feature = "uds"))]
use std::path::PathBuf;

#[derive(Clone)]
pub enum CommonAddr {
    SocketAddr(SocketAddr),
    DomainName(String, u16),
    #[cfg(all(unix, feature = "uds"))]
    UnixSocketPath(PathBuf),
}

impl Display for CommonAddr {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::SocketAddr(sockaddr) => write!(f, "{}", sockaddr),
            Self::DomainName(addr, port) => write!(f, "{}:{}", addr, port),
            #[cfg(all(unix, feature = "uds"))]
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

#[cfg(feature = "quic")]
use crate::transport::quic;

pub enum MaybeQuic<L> {
    #[cfg(feature = "quic")]
    Quic(quic::RawAcceptor),
    Other(L),
}

impl<L> MaybeQuic<L> {
    #[cfg(feature = "quic")]
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

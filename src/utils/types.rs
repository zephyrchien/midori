use std::fmt::{self, Display, Formatter};
use std::path::PathBuf;
use std::net::SocketAddr;

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

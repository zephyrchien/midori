use std::fmt::{self, Display, Formatter};
use std::path::PathBuf;
use std::net::SocketAddr;

#[derive(Clone)]
pub enum CommonAddr {
    SocketAddr(SocketAddr),
    DomainName(String, u16),
    UnixSocketPath(PathBuf),
}

impl Display for CommonAddr {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::SocketAddr(sockaddr) => write!(f, "{}", sockaddr),
            Self::DomainName(addr, port) => write!(f, "{}:{}", addr, port),
            Self::UnixSocketPath(path) => write!(f, "{}", path.display()),
        }
    }
}

/*
impl CommonAddr {
    pub async fn to_sockaddr(&self) -> io::Result<SocketAddr> {
        match self {
            Self::SocketAddr(sockaddr) => Ok(*sockaddr),
            Self::DomainName(addr, port) => {
                let ip = dns::resolve_async(addr).await?;
                Ok(SocketAddr::new(ip, *port))
            }
            _ => unreachable!(),
        }
    }
}

pub struct Endpoint<L: AsyncAccept, C: AsyncConnect> {
    pub lis: L,
    pub conn: C,
}

impl<L, C> Endpoint<L, C>
where
    L: AsyncAccept,
    C: AsyncConnect,
{
    fn new(lis: L, conn: C) -> Self { Endpoint { lis, conn } }
}
*/

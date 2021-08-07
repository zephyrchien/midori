use std::io;
use std::net::SocketAddr;
use futures::executor::block_on;

use log::debug;
use async_trait::async_trait;
use tokio::net::TcpListener;
#[cfg(all(unix, feature = "uds"))]
use tokio::net::UnixListener;

use super::PlainStream;
use crate::utils::CommonAddr;
use crate::transport::{AsyncAccept, Transport};

#[allow(clippy::upper_case_acronyms)]
pub enum PlainListener {
    TCP(TcpListener),
    #[cfg(all(unix, feature = "uds"))]
    UDS(UnixListener),
}

impl PlainListener {
    pub fn bind(addr: &CommonAddr) -> io::Result<PlainListener> {
        Ok(match addr {
            CommonAddr::SocketAddr(sockaddr) => {
                PlainListener::TCP(block_on(TcpListener::bind(sockaddr))?)
            }
            #[cfg(all(unix, feature = "uds"))]
            CommonAddr::UnixSocketPath(path) => {
                PlainListener::UDS(UnixListener::bind(path)?)
            }
            _ => unreachable!(),
        })
    }
    pub async fn accept_plain(&self) -> io::Result<(PlainStream, SocketAddr)> {
        Ok(match self {
            PlainListener::TCP(x) => {
                let (stream, sockaddr) = x.accept().await?;
                debug!("tcp accept <- {}", &sockaddr);
                stream.set_nodelay(true)?;
                (PlainStream::TCP(stream), sockaddr)
            }
            #[cfg(all(unix, feature = "uds"))]
            PlainListener::UDS(x) => {
                let (stream, path) = x.accept().await?;
                debug!("uds accept <- {:?}", path);
                let sockaddr = crate::utils::empty_sockaddr_v4();
                (PlainStream::UDS(stream), sockaddr)
            }
        })
    }
}

pub struct Acceptor {
    lis: PlainListener,
    addr: CommonAddr,
}

impl Acceptor {
    pub fn new(lis: PlainListener, addr: CommonAddr) -> Self {
        Acceptor { lis, addr }
    }
    #[cfg(target_os = "linux")]
    #[inline]
    pub fn inner(&self) -> &PlainListener { &self.lis }
}

#[async_trait]
impl AsyncAccept for Acceptor {
    const TRANS: Transport = Transport::TCP;

    const SCHEME: &'static str = "tcp";

    type IO = PlainStream;
    type Base = PlainStream;

    #[inline]
    fn addr(&self) -> &CommonAddr { &self.addr }

    #[inline]
    async fn accept_base(&self) -> io::Result<(Self::Base, SocketAddr)> {
        self.lis.accept_plain().await
    }

    #[inline]
    async fn accept(&self, base: Self::Base) -> io::Result<Self::IO> {
        // fake accept
        Ok(base)
    }
}

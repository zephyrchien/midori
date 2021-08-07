use std::io;
use std::net::SocketAddr;

use log::debug;
use async_trait::async_trait;

use tokio::net::TcpStream;
#[cfg(all(unix, feature = "uds"))]
use tokio::net::UnixStream;

use super::PlainStream;
use crate::dns;
use crate::utils::CommonAddr;
use crate::transport::{AsyncConnect, Transport};

#[derive(Clone)]
pub struct Connector {
    addr: CommonAddr,
}

impl Connector {
    pub fn new(addr: CommonAddr) -> Self { Connector { addr } }

    pub async fn connect_plain(&self) -> io::Result<PlainStream> {
        let stream = match &self.addr {
            CommonAddr::DomainName(addr, port) => {
                let ip = dns::resolve_async(addr).await?;
                let sockaddr = SocketAddr::new(ip, *port);
                debug!("tcp connect -> {}", &sockaddr);
                PlainStream::TCP(TcpStream::connect(sockaddr).await?)
            }
            CommonAddr::SocketAddr(sockaddr) => {
                debug!("tcp connect -> {}", sockaddr);
                PlainStream::TCP(TcpStream::connect(sockaddr).await?)
            }
            #[cfg(all(unix, feature = "uds"))]
            CommonAddr::UnixSocketPath(path) => {
                debug!("uds connect -> {:?}", path);
                PlainStream::UDS(UnixStream::connect(path).await?)
            }
        };
        stream.set_no_delay(true)?;
        Ok(stream)
    }
}

#[async_trait]
impl AsyncConnect for Connector {
    const TRANS: Transport = Transport::TCP;

    const SCHEME: &'static str = "tcp";

    type IO = PlainStream;

    #[inline]
    fn addr(&self) -> &CommonAddr { &self.addr }

    fn clear_reuse(&self) {}

    #[inline]
    async fn connect(&self) -> io::Result<Self::IO> {
        self.connect_plain().await
    }
}

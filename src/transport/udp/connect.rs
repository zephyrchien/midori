use std::io;
use std::net::SocketAddr;

use log::debug;

use tokio::net::UdpSocket;

use async_trait::async_trait;

use super::{UdpStream, Client};
use crate::transport::{AsyncConnect, Transport};
use crate::dns;
use crate::utils::{self, CommonAddr};

pub struct Connector {
    addr: CommonAddr,
}

impl Connector {
    #[inline]
    pub fn new(addr: CommonAddr) -> Self { Connector { addr } }
}

#[async_trait]
impl AsyncConnect for Connector {
    const TRANS: Transport = Transport::UDP;

    const SCHEME: &'static str = "udp";

    type IO = UdpStream<Client>;

    #[inline]
    fn addr(&self) -> &CommonAddr { &self.addr }

    fn clear_reuse(&self) {}

    async fn connect(&self) -> io::Result<Self::IO> {
        let connect_addr = match &self.addr {
            CommonAddr::SocketAddr(sockaddr) => *sockaddr,
            CommonAddr::DomainName(addr, port) => {
                let ip = dns::resolve_async(addr).await?;
                SocketAddr::new(ip, *port)
            }
            #[cfg(all(unix, feature = "uds"))]
            CommonAddr::UnixSocketPath(_) => unreachable!(),
        };
        let bind_addr = if connect_addr.is_ipv4() {
            utils::empty_sockaddr_v4()
        } else {
            utils::empty_sockaddr_v6()
        };
        debug!("udp connect {} -> {}", &bind_addr, &connect_addr);
        let socket = UdpSocket::bind(&bind_addr).await?;
        Ok(UdpStream::new(socket, Client(connect_addr)))
    }
}

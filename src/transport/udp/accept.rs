use std::io;
use std::net::SocketAddr;

use log::debug;
use socket2::{Socket, Type, Domain, SockAddr};
use async_trait::async_trait;

use tokio::net::UdpSocket;

use super::{UdpStream, Server};
use crate::dns;
use crate::utils::CommonAddr;
use crate::transport::{AsyncAccept, Transport};

pub struct Acceptor {
    addr: CommonAddr,
}

impl Acceptor {
    pub fn new(addr: CommonAddr) -> Self { Acceptor { addr } }
}

#[async_trait]
impl AsyncAccept for Acceptor {
    const TRANS: Transport = Transport::UDP;

    const SCHEME: &'static str = "udp";

    type IO = UdpStream<Server>;

    type Base = UdpStream<Server>;

    fn addr(&self) -> &CommonAddr { &self.addr }

    async fn accept_base(&self) -> io::Result<(Self::Base, SocketAddr)> {
        let bind_addr = match &self.addr {
            CommonAddr::SocketAddr(sockaddr) => *sockaddr,
            CommonAddr::DomainName(addr, port) => {
                let ip = dns::resolve_async(addr).await?;
                SocketAddr::new(ip, *port)
            }
            #[cfg(all(unix, feature = "uds"))]
            CommonAddr::UnixSocketPath(_) => unreachable!(),
        };
        let socket = new_udp_socket(bind_addr)?;
        let mut buffer = [0u8; 0];
        let (_, connect_addr) = socket.peek_from(&mut buffer).await?;
        debug!("udp accept {} <- {}", &bind_addr, &connect_addr);
        socket.connect(&connect_addr).await?;
        Ok((UdpStream::new(socket, Server {}), connect_addr))
    }

    #[inline]
    async fn accept(&self, base: Self::Base) -> io::Result<Self::IO> {
        Ok(base)
    }
}

fn new_udp_socket(sockaddr: SocketAddr) -> io::Result<UdpSocket> {
    let socket = Socket::new(
        if sockaddr.is_ipv4() {
            Domain::IPV4
        } else {
            Domain::IPV6
        },
        Type::DGRAM,
        None,
    )?;
    socket.set_nonblocking(true).unwrap();
    socket.set_reuse_address(true).unwrap();
    socket.bind(&SockAddr::from(sockaddr))?;
    UdpSocket::from_std(socket.into())
}

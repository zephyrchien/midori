use std::io;
use std::cmp::min;
use std::pin::Pin;
use std::task::{Poll, Context};
use std::net::SocketAddr;
use futures::ready;

use bytes::BytesMut;
use socket2::{Socket, Type, Domain, SockAddr};
use tokio::net::UdpSocket;
use tokio::io::{AsyncRead, AsyncWrite};
use async_trait::async_trait;

use super::{AsyncAccept, AsyncConnect, IOStream, Transport};
use crate::dns;
use crate::utils::{self, CommonAddr, UDP_BUF_SIZE};

pub struct UdpStream {
    io: UdpSocket,
    buffer: BytesMut,
}

impl IOStream for UdpStream {}

impl UdpStream {
    #[inline]
    fn new(io: UdpSocket) -> Self {
        UdpStream {
            io,
            buffer: BytesMut::with_capacity(UDP_BUF_SIZE),
        }
    }
}

impl AsyncRead for UdpStream {
    #[inline]
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        if !self.buffer.is_empty() {
            let to_read = min(buf.remaining(), self.buffer.len());
            let data = self.buffer.split_to(to_read);
            buf.put_slice(&data[..to_read]);
            return Poll::Ready(Ok(()));
        };
        Poll::Ready(ready!(self.io.poll_recv(cx, buf)))
    }
}

impl AsyncWrite for UdpStream {
    #[inline]
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize, io::Error>> {
        self.io.poll_send(cx, buf)
    }

    #[inline]
    fn poll_flush(
        self: Pin<&mut Self>,
        _: &mut Context<'_>,
    ) -> Poll<Result<(), io::Error>> {
        Poll::Ready(Ok(()))
    }

    #[inline]
    fn poll_shutdown(
        self: Pin<&mut Self>,
        _: &mut Context<'_>,
    ) -> Poll<Result<(), io::Error>> {
        Poll::Ready(Ok(()))
    }
}

// Connector
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

    type IO = UdpStream;

    #[inline]
    fn addr(&self) -> &CommonAddr { &self.addr }

    async fn connect(&self) -> io::Result<Self::IO> {
        let connect_addr = match &self.addr {
            CommonAddr::SocketAddr(sockaddr) => *sockaddr,
            CommonAddr::DomainName(addr, port) => {
                let ip = dns::resolve_async(addr).await?;
                SocketAddr::new(ip, *port)
            }
            #[cfg(unix)]
            CommonAddr::UnixSocketPath(_) => unreachable!(),
        };
        let bind_addr = if connect_addr.is_ipv4() {
            utils::empty_sockaddr_v4()
        } else {
            utils::empty_sockaddr_v6()
        };
        let socket = UdpSocket::bind(&bind_addr).await?;
        socket.connect(&connect_addr).await?;
        Ok(UdpStream::new(socket))
    }
}

// Acceptor
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

    type IO = UdpStream;

    type Base = UdpStream;

    fn addr(&self) -> &CommonAddr { &self.addr }

    async fn accept_base(&self) -> io::Result<(Self::Base, SocketAddr)> {
        let mut buffer = BytesMut::with_capacity(UDP_BUF_SIZE);
        let bind_addr = match &self.addr {
            CommonAddr::SocketAddr(sockaddr) => *sockaddr,
            CommonAddr::DomainName(addr, port) => {
                let ip = dns::resolve_async(addr).await?;
                SocketAddr::new(ip, *port)
            }
            #[cfg(unix)]
            CommonAddr::UnixSocketPath(_) => unreachable!(),
        };
        let socket = new_udp_socket(bind_addr)?;
        let (_, connect_addr) = socket.recv_from(&mut buffer).await?;
        socket.connect(&connect_addr).await?;
        Ok((UdpStream::new(socket), connect_addr))
    }

    #[inline]
    async fn accept(&self, base: Self::Base) -> io::Result<Self::IO> {
        Ok(base)
    }
}

#[inline]
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

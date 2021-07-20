use std::io;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::net::SocketAddr;
use futures::executor::block_on;

use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::{TcpStream, TcpListener};
use async_trait::async_trait;

use super::{AsyncConnect, AsyncAccept, IOStream, Transport};
use crate::dns;
use crate::utils::{self, CommonAddr};

#[cfg(unix)]
use std::os::unix::io::{AsRawFd, RawFd};
#[cfg(unix)]
use tokio::net::{UnixStream, UnixListener};

#[allow(clippy::upper_case_acronyms)]
pub enum PlainStream {
    TCP(TcpStream),
    #[cfg(unix)]
    UDS(UnixStream),
}

pub struct ReadHalf<'a>(&'a PlainStream);

pub struct WriteHalf<'a>(&'a PlainStream);

impl IOStream for PlainStream {}

#[cfg(unix)]
impl AsRawFd for PlainStream {
    fn as_raw_fd(&self) -> RawFd {
        match self {
            Self::TCP(x) => x.as_raw_fd(),
            #[cfg(unix)]
            Self::UDS(x) => x.as_raw_fd(),
        }
    }
}

impl AsRef<PlainStream> for ReadHalf<'_> {
    fn as_ref(&self) -> &PlainStream { self.0 }
}

impl AsRef<PlainStream> for WriteHalf<'_> {
    fn as_ref(&self) -> &PlainStream { self.0 }
}

impl PlainStream {
    pub fn set_no_delay(&self, nodelay: bool) -> io::Result<()> {
        match self {
            Self::TCP(x) => x.set_nodelay(nodelay),
            #[cfg(unix)]
            _ => Ok(()),
        }
    }
    #[cfg(target_os = "linux")] // unused unless meet zero-copy
    pub fn split(&mut self) -> (ReadHalf<'_>, WriteHalf<'_>) {
        (ReadHalf(&*self), WriteHalf(&*self))
    }
}

impl AsyncRead for PlainStream {
    #[inline]
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        match &mut self.get_mut() {
            Self::TCP(x) => Pin::new(x).poll_read(cx, buf),
            #[cfg(unix)]
            Self::UDS(x) => Pin::new(x).poll_read(cx, buf),
        }
    }
}

impl AsyncRead for ReadHalf<'_> {
    #[inline]
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        Pin::new(unsafe { utils::const_cast(self.get_mut().0) })
            .poll_read(cx, buf)
    }
}

impl AsyncWrite for PlainStream {
    #[inline]
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize, io::Error>> {
        match &mut self.get_mut() {
            Self::TCP(x) => Pin::new(x).poll_write(cx, buf),
            #[cfg(unix)]
            Self::UDS(x) => Pin::new(x).poll_write(cx, buf),
        }
    }

    #[inline]
    fn poll_flush(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), io::Error>> {
        match &mut self.get_mut() {
            Self::TCP(x) => Pin::new(x).poll_flush(cx),
            #[cfg(unix)]
            Self::UDS(x) => Pin::new(x).poll_flush(cx),
        }
    }

    #[inline]
    fn poll_shutdown(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), io::Error>> {
        match &mut self.get_mut() {
            Self::TCP(x) => Pin::new(x).poll_shutdown(cx),
            #[cfg(unix)]
            Self::UDS(x) => Pin::new(x).poll_shutdown(cx),
        }
    }
}

impl AsyncWrite for WriteHalf<'_> {
    #[inline]
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize, io::Error>> {
        Pin::new(unsafe { utils::const_cast(self.get_mut().0) })
            .poll_write(cx, buf)
    }

    #[inline]
    fn poll_flush(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), io::Error>> {
        Pin::new(unsafe { utils::const_cast(self.get_mut().0) }).poll_flush(cx)
    }

    #[inline]
    fn poll_shutdown(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), io::Error>> {
        Pin::new(unsafe { utils::const_cast(self.get_mut().0) })
            .poll_shutdown(cx)
    }
}

// Plain Connector
#[derive(Clone)]
pub struct Connector {
    addr: CommonAddr,
}

impl Connector {
    pub fn new(addr: CommonAddr) -> Self { Connector { addr } }
}

#[async_trait]
impl AsyncConnect for Connector {
    const TRANS: Transport = Transport::TCP;

    const SCHEME: &'static str = "tcp";

    type IO = PlainStream;

    fn addr(&self) -> &CommonAddr { &self.addr }

    async fn connect(&self) -> io::Result<Self::IO> {
        let stream = match &self.addr {
            CommonAddr::DomainName(addr, port) => {
                let ip = dns::resolve_async(addr).await?;
                let sockaddr = SocketAddr::new(ip, *port);
                PlainStream::TCP(TcpStream::connect(sockaddr).await?)
            }
            CommonAddr::SocketAddr(sockaddr) => {
                PlainStream::TCP(TcpStream::connect(sockaddr).await?)
            }
            #[cfg(unix)]
            CommonAddr::UnixSocketPath(path) => {
                PlainStream::UDS(UnixStream::connect(path).await?)
            }
        };
        stream.set_no_delay(true)?;
        Ok(stream)
    }
}

// Plain Acceptor
#[allow(clippy::upper_case_acronyms)]
pub enum PlainListener {
    TCP(TcpListener),
    #[cfg(unix)]
    UDS(UnixListener),
}

impl PlainListener {
    pub fn bind(addr: &CommonAddr) -> io::Result<PlainListener> {
        Ok(match addr {
            CommonAddr::SocketAddr(sockaddr) => {
                PlainListener::TCP(block_on(TcpListener::bind(sockaddr))?)
            }
            #[cfg(unix)]
            CommonAddr::UnixSocketPath(path) => {
                PlainListener::UDS(UnixListener::bind(path)?)
            }
            _ => unreachable!(),
        })
    }
    pub async fn accept_plain(&self) -> io::Result<(PlainStream, SocketAddr)> {
        #[cfg(unix)]
        use std::net::{IpAddr, Ipv4Addr};
        Ok(match self {
            PlainListener::TCP(x) => {
                let (stream, sockaddr) = x.accept().await?;
                stream.set_nodelay(true)?;
                (PlainStream::TCP(stream), sockaddr)
            }
            #[cfg(unix)]
            PlainListener::UDS(x) => {
                let (stream, _) = x.accept().await?;
                let sockaddr =
                    SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), 0);
                (PlainStream::UDS(stream), sockaddr)
            }
        })
    }
}

#[derive(Clone)]
pub struct Acceptor {
    addr: CommonAddr,
}

impl Acceptor {
    pub fn new(addr: CommonAddr) -> Self { Acceptor { addr } }
}

#[async_trait]
impl AsyncAccept for Acceptor {
    const TRANS: Transport = Transport::TCP;

    const SCHEME: &'static str = "tcp";

    type IO = PlainStream;

    fn addr(&self) -> &CommonAddr { &self.addr }

    async fn accept(
        &self,
        res: (PlainStream, SocketAddr),
    ) -> io::Result<(Self::IO, SocketAddr)> {
        // fake accept
        Ok(res)
    }
}

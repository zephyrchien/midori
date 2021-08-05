use std::io;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::net::SocketAddr;
use futures::executor::block_on;

use log::debug;
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
}

#[cfg(target_os = "linux")]
pub mod linux_ext {
    use tokio::io::Interest;
    use super::*;

    #[inline]
    pub fn split(x: &mut PlainStream) -> (ReadHalf, WriteHalf) {
        (ReadHalf(&*x), WriteHalf(&*x))
    }

    // tokio >= 1.9.0
    #[inline]
    pub fn try_io<R>(
        x: &PlainStream,
        interest: Interest,
        f: impl FnOnce() -> io::Result<R>,
    ) -> io::Result<R> {
        match x {
            PlainStream::TCP(x) => x.try_io(interest, f),
            PlainStream::UDS(x) => x.try_io(interest, f),
        }
    }

    #[inline]
    pub async fn readable(x: &PlainStream) -> io::Result<()> {
        match x {
            PlainStream::TCP(x) => x.readable().await,
            PlainStream::UDS(x) => x.readable().await,
        }
    }

    #[inline]
    pub async fn writable(x: &PlainStream) -> io::Result<()> {
        match x {
            PlainStream::TCP(x) => x.writable().await,
            PlainStream::UDS(x) => x.writable().await,
        }
    }

    #[inline]
    pub fn clear_readiness(x: &PlainStream, interest: Interest) {
        use {io::Error, io::ErrorKind::WouldBlock};
        let _ = try_io(x, interest, || {
            Err(Error::new(WouldBlock, "")) as io::Result<()>
        });
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

    #[inline]
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
            #[cfg(unix)]
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

    async fn connect(&self) -> io::Result<Self::IO> {
        self.connect_plain().await
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
        Ok(match self {
            PlainListener::TCP(x) => {
                let (stream, sockaddr) = x.accept().await?;
                debug!("tcp accept <- {}", &sockaddr);
                stream.set_nodelay(true)?;
                (PlainStream::TCP(stream), sockaddr)
            }
            #[cfg(unix)]
            PlainListener::UDS(x) => {
                let (stream, path) = x.accept().await?;
                debug!("uds accept <- {:?}", path);
                let sockaddr = utils::empty_sockaddr_v4();
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

    async fn accept(&self, base: Self::Base) -> io::Result<Self::IO> {
        // fake accept
        Ok(base)
    }
}

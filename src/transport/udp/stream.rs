use std::io::Result;
use std::pin::Pin;
use std::task::{Poll, Context};
use std::net::SocketAddr;

use tokio::net::UdpSocket;
use tokio::io::{AsyncRead, AsyncWrite};

use crate::transport::IOStream;

// client does not perform connect
// so that it can recv from any remote addr
pub struct Client(pub SocketAddr);
pub struct Server {}
pub struct UdpStream<T> {
    io: UdpSocket,
    role: T,
}

impl IOStream for UdpStream<Client> {}
impl IOStream for UdpStream<Server> {}

impl<T> UdpStream<T> {
    #[inline]
    pub fn new(io: UdpSocket, role: T) -> Self { UdpStream { io, role } }
}

impl AsyncRead for UdpStream<Server> {
    #[inline]
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> Poll<Result<()>> {
        self.io.poll_recv(cx, buf)
    }
}

impl AsyncRead for UdpStream<Client> {
    #[inline]
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> Poll<Result<()>> {
        self.io.poll_recv_from(cx, buf).map(|_| Ok(()))
    }
}

impl AsyncWrite for UdpStream<Server> {
    #[inline]
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize>> {
        self.io.poll_send(cx, buf)
    }

    #[inline]
    fn poll_flush(
        self: Pin<&mut Self>,
        _: &mut Context<'_>,
    ) -> Poll<Result<()>> {
        Poll::Ready(Ok(()))
    }

    #[inline]
    fn poll_shutdown(
        self: Pin<&mut Self>,
        _: &mut Context<'_>,
    ) -> Poll<Result<()>> {
        Poll::Ready(Ok(()))
    }
}

impl AsyncWrite for UdpStream<Client> {
    #[inline]
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize>> {
        self.io.poll_send_to(cx, buf, self.role.0)
    }

    #[inline]
    fn poll_flush(
        self: Pin<&mut Self>,
        _: &mut Context<'_>,
    ) -> Poll<Result<()>> {
        Poll::Ready(Ok(()))
    }

    #[inline]
    fn poll_shutdown(
        self: Pin<&mut Self>,
        _: &mut Context<'_>,
    ) -> Poll<Result<()>> {
        Poll::Ready(Ok(()))
    }
}

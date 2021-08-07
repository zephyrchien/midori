use std::io;
use std::cmp::min;
use std::pin::Pin;
use std::task::{Poll, Context};
use std::net::SocketAddr;
use futures::ready;

use bytes::BytesMut;

use tokio::net::UdpSocket;
use tokio::io::{AsyncRead, AsyncWrite};

use crate::utils::UDP_BUF_SIZE;
use crate::transport::IOStream;

// client does not perform connect
// so that it can recv from any remote addr
pub struct Client(pub SocketAddr);
pub struct Server {}
pub struct UdpStream<T> {
    io: UdpSocket,
    buffer: BytesMut,
    role: T,
}

impl IOStream for UdpStream<Client> {}
impl IOStream for UdpStream<Server> {}

impl<T> UdpStream<T> {
    #[inline]
    pub fn new(io: UdpSocket, role: T) -> Self {
        UdpStream {
            io,
            buffer: BytesMut::with_capacity(UDP_BUF_SIZE),
            role,
        }
    }
}

impl AsyncRead for UdpStream<Server> {
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

impl AsyncRead for UdpStream<Client> {
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
        Poll::Ready(ready!(self.io.poll_recv_from(cx, buf)).map(|_| ()))
    }
}

impl AsyncWrite for UdpStream<Server> {
    #[inline]
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        self.io.poll_send(cx, buf)
    }

    #[inline]
    fn poll_flush(
        self: Pin<&mut Self>,
        _: &mut Context<'_>,
    ) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }

    #[inline]
    fn poll_shutdown(
        self: Pin<&mut Self>,
        _: &mut Context<'_>,
    ) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }
}

impl AsyncWrite for UdpStream<Client> {
    #[inline]
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        self.io.poll_send_to(cx, buf, self.role.0)
    }

    #[inline]
    fn poll_flush(
        self: Pin<&mut Self>,
        _: &mut Context<'_>,
    ) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }

    #[inline]
    fn poll_shutdown(
        self: Pin<&mut Self>,
        _: &mut Context<'_>,
    ) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }
}

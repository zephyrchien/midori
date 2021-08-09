use std::io::Result;
use std::cmp::min;
use std::pin::Pin;
use std::task::{Poll, Context};
use std::net::SocketAddr;
use std::sync::Arc;
use futures::ready;

use bytes::{Bytes, BytesMut};

use tokio::net::UdpSocket;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::sync::mpsc::Receiver;

use crate::transport::IOStream;
use crate::utils::UDP_BUF_SIZE;

pub struct UdpClientStream {
    io: UdpSocket,
    remote: SocketAddr,
}

pub struct UdpServerStream {
    io: Arc<UdpSocket>,
    recv: Receiver<Bytes>,
    buffer: BytesMut,
    remote: SocketAddr,
}

impl IOStream for UdpClientStream {}
impl IOStream for UdpServerStream {}

impl UdpClientStream {
    #[inline]
    pub fn new(io: UdpSocket, remote: SocketAddr) -> Self {
        UdpClientStream { io, remote }
    }
}

impl UdpServerStream {
    #[inline]
    pub fn new(
        io: Arc<UdpSocket>,
        recv: Receiver<Bytes>,
        remote: SocketAddr,
    ) -> Self {
        UdpServerStream {
            io,
            recv,
            buffer: BytesMut::with_capacity(UDP_BUF_SIZE),
            remote,
        }
    }
}

impl AsyncRead for UdpServerStream {
    #[inline]
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> Poll<Result<()>> {
        if !self.buffer.is_empty() {
            let to_read = min(buf.remaining(), self.buffer.len());
            let data = self.buffer.split_to(to_read);
            buf.put_slice(&data[..to_read]);
            return Poll::Ready(Ok(()));
        };
        Poll::Ready(match ready!(self.recv.poll_recv(cx)) {
            Some(data) => {
                let to_read = min(buf.remaining(), data.len());
                buf.put_slice(&data[..to_read]);
                if data.len() > to_read {
                    self.buffer.extend_from_slice(&data[to_read..]);
                }
                Ok(())
            }
            // reads 0, EOF
            None => Ok(()),
        })
    }
}

impl AsyncRead for UdpClientStream {
    #[inline]
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> Poll<Result<()>> {
        self.io.poll_recv_from(cx, buf).map(|_| Ok(()))
    }
}

impl AsyncWrite for UdpServerStream {
    #[inline]
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize>> {
        self.io.poll_send_to(cx, buf, self.remote)
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

impl AsyncWrite for UdpClientStream {
    #[inline]
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize>> {
        self.io.poll_send_to(cx, buf, self.remote)
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

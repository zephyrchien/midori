use std::pin::Pin;
use std::task::{Poll, Context};
use std::io::Result;

use tokio::io::{AsyncRead, AsyncWrite};
use quinn::crypto::rustls::TlsSession;
use quinn::generic::{SendStream, RecvStream};

use crate::transport::IOStream;

pub struct QuicStream {
    send: SendStream<TlsSession>,
    recv: RecvStream<TlsSession>,
}

impl QuicStream {
    #[inline]
    pub fn new(
        send: SendStream<TlsSession>,
        recv: RecvStream<TlsSession>,
    ) -> Self {
        QuicStream { send, recv }
    }
}

impl IOStream for QuicStream {}

impl AsyncRead for QuicStream {
    #[inline]
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> Poll<Result<()>> {
        Pin::new(&mut self.recv).poll_read(cx, buf)
    }
}

impl AsyncWrite for QuicStream {
    #[inline]
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize>> {
        Pin::new(&mut self.send).poll_write(cx, buf)
    }

    #[inline]
    fn poll_flush(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<()>> {
        Pin::new(&mut self.send).poll_flush(cx)
    }

    #[inline]
    fn poll_shutdown(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<()>> {
        Pin::new(&mut self.send).poll_shutdown(cx)
    }
}

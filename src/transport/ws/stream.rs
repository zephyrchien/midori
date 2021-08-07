use std::cmp::min;
use std::pin::Pin;
use std::task::{Poll, Context};
use std::io::{Error, ErrorKind, Result};

use futures::ready;
use futures::sink::Sink;
use futures::stream::Stream;
use bytes::BytesMut;

use tokio::io::{AsyncRead, AsyncWrite};
use tokio_tungstenite::WebSocketStream as RawWebSocketStream;
use tokio_tungstenite::tungstenite;
use tungstenite::Message;

use crate::utils::WS_BUF_SIZE;
use crate::transport::IOStream;

pub struct WebSocketStream<S> {
    io: RawWebSocketStream<S>,
    buffer: BytesMut,
}

impl<S: IOStream> IOStream for WebSocketStream<S> {}

impl<S> WebSocketStream<S> {
    #[inline]
    pub fn new(io: RawWebSocketStream<S>) -> Self {
        WebSocketStream {
            io,
            buffer: BytesMut::with_capacity(WS_BUF_SIZE),
        }
    }
}

// impl AsyncRead, AsyncWrite
// borrowed from
// https://github.com/eycorsican/leaf/blob/master/leaf/src/proxy/ws/stream.rs
impl<S> AsyncRead for WebSocketStream<S>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
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
            //cx.waker().wake_by_ref();
            return Poll::Ready(Ok(()));
        };
        Poll::Ready(ready!(Pin::new(&mut self.io).poll_next(cx)).map_or(
            Err(Error::new(ErrorKind::ConnectionReset, "connection reset")),
            |item| {
                item.map_or_else(
                    |e| Err(Error::new(ErrorKind::Interrupted, e)),
                    |msg| match msg {
                        Message::Binary(data) => {
                            let to_read = min(buf.remaining(), data.len());
                            buf.put_slice(&data[..to_read]);
                            if data.len() > to_read {
                                self.buffer.extend_from_slice(&data[to_read..]);
                            }
                            Ok(())
                        }
                        Message::Close(_) => Ok(()),
                        _ => Err(Error::new(
                            ErrorKind::InvalidData,
                            "invalid frame",
                        )),
                    },
                )
            },
        ))
    }
}

impl<S> AsyncWrite for WebSocketStream<S>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    #[inline]
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context,
        buf: &[u8],
    ) -> Poll<Result<usize>> {
        ready!(Pin::new(&mut self.io)
            .poll_ready(cx)
            .map_err(|e| Error::new(ErrorKind::BrokenPipe, e)))?;

        let msg = Message::Binary(buf.to_vec());
        Pin::new(&mut self.io)
            .start_send(msg)
            .map_err(|e| Error::new(ErrorKind::BrokenPipe, e))?;

        Poll::Ready(Ok(buf.len()))
    }

    #[inline]
    fn poll_flush(
        mut self: Pin<&mut Self>,
        cx: &mut Context,
    ) -> Poll<Result<()>> {
        Pin::new(&mut self.io)
            .poll_flush(cx)
            .map_err(|e| Error::new(ErrorKind::BrokenPipe, e))
    }

    #[inline]
    fn poll_shutdown(
        mut self: Pin<&mut Self>,
        cx: &mut Context,
    ) -> Poll<Result<()>> {
        // send a close frame
        ready!(Pin::new(&mut self.io)
            .poll_ready(cx)
            .map_err(|e| Error::new(ErrorKind::BrokenPipe, e)))?;
        let _ = Pin::new(&mut self.io)
            .start_send(Message::Close(None))
            .map_err(|e| Error::new(ErrorKind::BrokenPipe, e));
        Pin::new(&mut self.io)
            .poll_close(cx)
            .map_err(|e| Error::new(ErrorKind::ConnectionAborted, e))
    }
}

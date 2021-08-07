use std::cmp::min;
use std::pin::Pin;
use std::task::{Poll, Context};
use std::io::{Result, Error, ErrorKind};

use futures::ready;
use bytes::{Bytes, BytesMut};

use tokio::io::{AsyncRead, AsyncWrite};
use h2::{SendStream, RecvStream};

use crate::transport::IOStream;
use crate::utils::H2_BUF_SIZE;

pub struct H2Stream {
    recv: RecvStream,
    send: SendStream<Bytes>,
    buffer: BytesMut,
}

impl H2Stream {
    #[inline]
    pub fn new(recv: RecvStream, send: SendStream<Bytes>) -> Self {
        H2Stream {
            recv,
            send,
            buffer: BytesMut::with_capacity(H2_BUF_SIZE),
        }
    }
}

impl IOStream for H2Stream {}

impl AsyncRead for H2Stream {
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
        Poll::Ready(match ready!(self.recv.poll_data(cx)) {
            Some(Ok(data)) => {
                let to_read = min(buf.remaining(), data.len());
                buf.put_slice(&data[..to_read]);
                // copy the left payload into buffer
                if data.len() > to_read {
                    self.buffer.extend_from_slice(&data[to_read..]);
                };
                // increase recv window
                self.recv
                    .flow_control()
                    .release_capacity(to_read)
                    .map_or_else(
                        |e| Err(Error::new(ErrorKind::ConnectionReset, e)),
                        |_| Ok(()),
                    )
            }
            // no more data frames
            // maybe trailer
            // or cancelled
            _ => Ok(()),
        })
    }
}

impl AsyncWrite for H2Stream {
    #[inline]
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize>> {
        self.send.reserve_capacity(buf.len());
        Poll::Ready(match ready!(self.send.poll_capacity(cx)) {
            Some(Ok(to_write)) => self
                .send
                .send_data(Bytes::from(buf[..to_write].to_owned()), false)
                .map_or_else(
                    |e| Err(Error::new(ErrorKind::BrokenPipe, e)),
                    |_| Ok(to_write),
                ),
            // is_send_streaming returns false
            // which indicates the state is
            // neither open nor half_close_remote
            _ => Err(Error::new(ErrorKind::BrokenPipe, "broken pipe")),
        })
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
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<()>> {
        self.send.reserve_capacity(0);
        Poll::Ready(ready!(self.send.poll_capacity(cx)).map_or(
            Err(Error::new(ErrorKind::BrokenPipe, "broken pipe")),
            |_| {
                self.send.send_data(Bytes::new(), true).map_or_else(
                    |e| Err(Error::new(ErrorKind::BrokenPipe, e)),
                    |_| Ok(()),
                )
            },
        ))
    }
}

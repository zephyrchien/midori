use std::io;
use std::cmp::min;
use std::pin::Pin;
use std::task::{Poll, Context};
use std::net::SocketAddr;
use futures::ready;
use futures::sink::Sink;
use futures::stream::Stream;

use bytes::BytesMut;
use http::StatusCode;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio_tungstenite::WebSocketStream;
use tokio_tungstenite::tungstenite;
use tungstenite::Message;
use tungstenite::protocol::WebSocketConfig;
use tungstenite::handshake::server::{Callback, Request, Response, ErrorResponse};
use async_trait::async_trait;

use super::{AsyncConnect, AsyncAccept, IOStream};
use super::plain::PlainStream;
use crate::utils::{self, CommonAddr};

pub struct WSStream<S> {
    io: WebSocketStream<S>,
    buffer: BytesMut,
}

impl<S: IOStream> IOStream for WSStream<S> {}

impl<S> WSStream<S> {
    pub fn new(io: WebSocketStream<S>) -> Self {
        WSStream {
            io,
            buffer: BytesMut::with_capacity(4096),
        }
    }
}

// impl AsyncRead, AsyncWrite
// borrowed from
// https://github.com/eycorsican/leaf/blob/master/leaf/src/proxy/ws/stream.rs
impl<S> AsyncRead for WSStream<S>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
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
            //cx.waker().wake_by_ref();
            return Poll::Ready(Ok(()));
        };
        Poll::Ready(ready!(Pin::new(&mut self.io).poll_next(cx)).map_or(
            Err(utils::new_io_err("broken pipe")),
            |item| {
                item.map_or(Err(utils::new_io_err("broken pipe")), |msg| {
                    match msg {
                        Message::Binary(data) => {
                            let to_read = min(buf.remaining(), data.len());
                            buf.put_slice(&data[..to_read]);
                            if data.len() > to_read {
                                self.buffer.extend_from_slice(&data[to_read..]);
                            }
                            Ok(())
                        }
                        Message::Close(_) => Ok(()),
                        _ => Err(utils::new_io_err("invalid frame")),
                    }
                })
            },
        ))
    }
}

impl<S> AsyncWrite for WSStream<S>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    #[inline]
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        ready!(Pin::new(&mut self.io)
            .poll_ready(cx)
            .map_err(|_| utils::new_io_err("broken_pipe")))?;

        let msg = Message::Binary(buf.to_vec());
        Pin::new(&mut self.io)
            .start_send(msg)
            .map_err(|_| utils::new_io_err("broken_pipe"))?;

        Poll::Ready(Ok(buf.len()))
    }

    #[inline]
    fn poll_flush(
        mut self: Pin<&mut Self>,
        cx: &mut Context,
    ) -> Poll<io::Result<()>> {
        Pin::new(&mut self.io)
            .poll_flush(cx)
            .map_err(|_| utils::new_io_err("broken_pipe"))
    }

    #[inline]
    fn poll_shutdown(
        mut self: Pin<&mut Self>,
        cx: &mut Context,
    ) -> Poll<io::Result<()>> {
        // send a close frame
        ready!(Pin::new(&mut self.io)
            .poll_ready(cx)
            .map_err(|e| utils::new_io_err(&e.to_string())))?;
        let _ = Pin::new(&mut self.io).start_send(Message::Close(None));
        Pin::new(&mut self.io)
            .poll_close(cx)
            .map_err(|e| utils::new_io_err(&e.to_string()))
    }
}

// WebSocket Connector
#[derive(Clone)]
pub struct Connector<T: AsyncConnect> {
    cc: T,
    req: String,
    config: Option<WebSocketConfig>,
}

impl<T: AsyncConnect> Connector<T> {
    pub fn new(cc: T, req: String) -> Self {
        Connector {
            cc,
            req,
            config: None,
        }
    }
}

#[async_trait]
impl<T: AsyncConnect> AsyncConnect for Connector<T> {
    type IO = WSStream<T::IO>;

    fn addr(&self) -> &CommonAddr { self.cc.addr() }

    async fn connect(&self) -> io::Result<Self::IO> {
        let stream = self.cc.connect().await?;
        tokio_tungstenite::client_async_with_config(
            &self.req,
            stream,
            self.config,
        )
        .await
        .map_or_else(
            |e| {
                println!("{}", e);
                Err(utils::new_io_err(&e.to_string()))
            },
            |(ws, _)| Ok(WSStream::new(ws)),
        )
    }
}

// WebSocket Acceptor
#[derive(Clone)]
pub struct Acceptor<T: AsyncAccept> {
    lis: T,
    path: String,
    config: Option<WebSocketConfig>,
}

impl<T: AsyncAccept> Acceptor<T> {
    pub fn new(lis: T, path: String) -> Self {
        Acceptor {
            lis,
            path,
            config: None,
        }
    }
}

struct RequestHook {
    path: String,
}

impl Callback for RequestHook {
    #[inline]
    fn on_request(
        self,
        request: &Request,
        response: Response,
    ) -> Result<Response, ErrorResponse> {
        if request.uri().path() == self.path {
            Ok(response)
        } else {
            let mut response = ErrorResponse::new(None);
            *response.status_mut() = StatusCode::NOT_FOUND;
            Err(response)
        }
    }
}

#[async_trait]
impl<T: AsyncAccept> AsyncAccept for Acceptor<T> {
    type IO = WSStream<T::IO>;

    fn addr(&self) -> &CommonAddr { self.lis.addr() }

    async fn accept(
        &self,
        res: (PlainStream, SocketAddr),
    ) -> io::Result<(Self::IO, SocketAddr)> {
        let (stream, addr) = self.lis.accept(res).await?;
        let hook = RequestHook {
            path: self.path.clone(),
        };
        tokio_tungstenite::accept_hdr_async_with_config(
            stream,
            hook,
            self.config,
        )
        .await
        .map_or_else(
            |e| Err(utils::new_io_err(&e.to_string())),
            |ws| Ok((WSStream::new(ws), addr)),
        )
    }
}

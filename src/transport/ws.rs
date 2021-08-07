use std::cmp::min;
use std::pin::Pin;
use std::task::{Poll, Context};
use std::io::{Error, ErrorKind, Result};
use std::net::SocketAddr;
use futures::ready;
use futures::sink::Sink;
use futures::stream::Stream;

use log::debug;
use bytes::BytesMut;
use http::{Uri, StatusCode};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio_tungstenite::WebSocketStream as RawWebSocketStream;
use tokio_tungstenite::tungstenite;
use tungstenite::Message;
use tungstenite::protocol::WebSocketConfig;
use tungstenite::handshake::server::{Callback, Request, Response, ErrorResponse};
use async_trait::async_trait;

use super::{AsyncConnect, AsyncAccept, IOStream, Transport};
use crate::utils::{CommonAddr, WS_BUF_SIZE};

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

// WebSocket Connector
pub struct Connector<T: AsyncConnect> {
    cc: T,
    uri: Uri,
    config: Option<WebSocketConfig>,
}

impl<T: AsyncConnect> Connector<T> {
    pub fn new(cc: T, path: String) -> Self {
        let authority = cc.addr().to_string();
        Connector {
            cc,
            uri: Uri::builder()
                .scheme(Self::SCHEME)
                .authority(authority.as_str())
                .path_and_query(path)
                .build()
                .unwrap(),
            config: None,
        }
    }
}

#[async_trait]
impl<T: AsyncConnect> AsyncConnect for Connector<T> {
    const TRANS: Transport = Transport::WS;

    const SCHEME: &'static str = match T::TRANS {
        Transport::TLS => "wss",
        _ => "ws",
    };

    type IO = WebSocketStream<T::IO>;

    #[inline]
    fn addr(&self) -> &CommonAddr { self.cc.addr() }

    fn clear_reuse(&self) {}

    async fn connect(&self) -> Result<Self::IO> {
        let stream = self.cc.connect().await?;
        debug!("ws connect ->");
        tokio_tungstenite::client_async_with_config(
            &self.uri,
            stream,
            self.config,
        )
        .await
        .map_or_else(
            |e| Err(Error::new(ErrorKind::ConnectionRefused, e)),
            |(ws, _)| Ok(WebSocketStream::new(ws)),
        )
    }
}

// WebSocket Acceptor
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
    fn on_request(
        self,
        request: &Request,
        response: Response,
    ) -> std::result::Result<Response, ErrorResponse> {
        if request.uri().path() == self.path {
            debug!("check request path -- ok");
            Ok(response)
        } else {
            debug!("check request path -- not found");
            let mut response = ErrorResponse::new(None);
            *response.status_mut() = StatusCode::NOT_FOUND;
            Err(response)
        }
    }
}

#[async_trait]
impl<T: AsyncAccept> AsyncAccept for Acceptor<T> {
    const TRANS: Transport = Transport::WS;

    const SCHEME: &'static str = match T::TRANS {
        Transport::TLS => "wss",
        _ => "ws",
    };

    type IO = WebSocketStream<T::IO>;

    type Base = T::Base;

    #[inline]
    fn addr(&self) -> &CommonAddr { self.lis.addr() }

    #[inline]
    async fn accept_base(&self) -> Result<(Self::Base, SocketAddr)> {
        self.lis.accept_base().await
    }

    async fn accept(&self, base: Self::Base) -> Result<Self::IO> {
        let stream = self.lis.accept(base).await?;
        debug!("ws accept <-");

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
            |e| Err(Error::new(ErrorKind::ConnectionAborted, e)),
            |ws| Ok(WebSocketStream::new(ws)),
        )
    }
}

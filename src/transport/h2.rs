use std::io;
use std::cmp::min;
use std::pin::Pin;
use std::task::{Poll, Context};
use std::net::SocketAddr;
use futures::ready;

use bytes::{Bytes, BytesMut};
use http::{Uri, Version, StatusCode, Request, Response};
use tokio::io::{AsyncRead, AsyncWrite};
use h2::{client, server};
use h2::{SendStream, RecvStream};

use async_trait::async_trait;

use super::{AsyncConnect, AsyncAccept, IOStream, Transport};
use super::plain::PlainStream;
use crate::utils::{self, CommonAddr, H2_BUF_SIZE};

pub struct H2Stream {
    recv: RecvStream,
    send: SendStream<Bytes>,
    buffer: BytesMut,
}

impl H2Stream {
    pub fn new(
        recv: RecvStream,
        send: SendStream<Bytes>,
        buffer: BytesMut,
    ) -> Self {
        H2Stream { recv, send, buffer }
    }
}

impl IOStream for H2Stream {}

impl AsyncRead for H2Stream {
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
        Poll::Ready(match ready!(self.recv.poll_data(cx)) {
            Some(data) => {
                let data = data.unwrap();
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
                    .map_err(|e| utils::new_io_err(&e.to_string()))
            }
            // no more data frames
            // maybe trailer
            None => Ok(()),
        })
    }
}

impl AsyncWrite for H2Stream {
    #[inline]
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize, io::Error>> {
        self.send.reserve_capacity(buf.len());
        Poll::Ready(match ready!(self.send.poll_capacity(cx)) {
            Some(to_write) => {
                let to_write = to_write.unwrap();
                self.send
                    .send_data(Bytes::from(buf[..to_write].to_owned()), false)
                    .map_or_else(
                        |e| Err(utils::new_io_err(&e.to_string())),
                        |_| Ok(to_write),
                    )
            }
            // is_send_streaming returns false
            // which indicates the state is
            // neither open nor half_close_remote
            None => Err(utils::new_io_err("broken pipe")),
        })
    }

    #[inline]
    fn poll_flush(
        self: Pin<&mut Self>,
        _: &mut Context<'_>,
    ) -> Poll<Result<(), io::Error>> {
        Poll::Ready(Ok(()))
    }

    #[inline]
    fn poll_shutdown(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), io::Error>> {
        self.send.reserve_capacity(0);
        Poll::Ready(ready!(self.send.poll_capacity(cx)).map_or(
            Err(utils::new_io_err("broken pipe")),
            |_| {
                self.send.send_data(Bytes::new(), true).map_or_else(
                    |e| Err(utils::new_io_err(&e.to_string())),
                    |_| Ok(()),
                )
            },
        ))
    }
}

// HTTP2 Connector
#[derive(Clone)]
pub struct Connector<T: AsyncConnect> {
    cc: T,
    uri: Uri,
    allow_push: bool,
}

impl<T: AsyncConnect> Connector<T> {
    pub fn new(cc: T, path: String, allow_push: bool) -> Self {
        let authority = cc.addr().to_string();
        Connector {
            cc,
            uri: Uri::builder()
                .scheme(Self::SCHEME)
                .authority(authority.as_str())
                .path_and_query(path)
                .build()
                .unwrap(),
            allow_push,
        }
    }
}

#[async_trait]
impl<T: AsyncConnect> AsyncConnect for Connector<T> {
    const TRANS: Transport = Transport::H2;

    const SCHEME: &'static str = match T::TRANS {
        Transport::TLS => "https",
        _ => "http",
    };

    type IO = H2Stream;

    fn addr(&self) -> &CommonAddr { self.cc.addr() }

    async fn connect(&self) -> io::Result<Self::IO> {
        let stream = self.cc.connect().await?;
        // establish a new connection
        let (h2, conn) = client::Builder::new()
            .enable_push(self.allow_push)
            .handshake(stream)
            .await
            .map_err(|e| utils::new_io_err(&e.to_string()))?;

        tokio::spawn(conn);

        // create a new stream
        let mut client = h2
            .ready()
            .await
            .map_err(|e| utils::new_io_err(&e.to_string()))?;

        // request with a send stream
        let (response, send) = client
            .send_request(
                Request::builder()
                    .uri(&self.uri)
                    .version(Version::HTTP_2)
                    .body(())
                    .unwrap(),
                false,
            )
            .map_err(|e| utils::new_io_err(&e.to_string()))?;

        /*
        // prepare to recv server push
        let mut pushes = response.push_promises();
        let push = pushes.push_promise();
        */

        // get recv stream from response body
        let recv = response
            .await
            .map_err(|e| utils::new_io_err(&e.to_string()))?
            .into_body();

        /*
        // try recv stream from push body
        // NOT SOLVED:
        // ALWAYS HANG when try to
        // resolve the recv stream
        // from the pushed response
        if self.allow_push {
            if let Some(Ok(push)) = push.await {
                let (_, push) = push.into_parts();
                // HANG
                if let Ok(response) = push.await {
                    let recv = response.into_body();
                    return Ok(H2Stream::new(
                        recv,
                        send,
                        BytesMut::with_capacity(H2_BUF_SIZE),
                    ));
                }
            }
        }
        */

        // fallback
        Ok(H2Stream::new(
            recv,
            send,
            BytesMut::with_capacity(H2_BUF_SIZE),
        ))
    }
}

// HTTP2 Acceptor
#[derive(Clone)]
pub struct Acceptor<T: AsyncAccept> {
    lis: T,
    path: String,
    server_push: bool,
}

impl<T: AsyncAccept> Acceptor<T> {
    pub fn new(lis: T, path: String, server_push: bool) -> Self {
        Acceptor {
            lis,
            path,
            server_push,
        }
    }
}

#[async_trait]
impl<T: AsyncAccept> AsyncAccept for Acceptor<T> {
    const MUX: bool = true;

    const TRANS: Transport = Transport::H2;

    const SCHEME: &'static str = match T::TRANS {
        Transport::TLS => "https",
        _ => "http",
    };

    type IO = H2Stream;

    fn addr(&self) -> &CommonAddr { self.lis.addr() }

    async fn accept(
        &self,
        res: (PlainStream, SocketAddr),
    ) -> io::Result<(Self::IO, SocketAddr)> {
        let (stream, addr) = self.lis.accept(res).await?;
        // establish a new connection
        let mut conn = server::handshake(stream)
            .await
            .map_err(|e| utils::new_io_err(&e.to_string()))?;

        // accept a new stream
        let (request, mut response) = conn
            .accept()
            .await
            .unwrap()
            .map_err(|e| utils::new_io_err(&e.to_string()))?;

        tokio::spawn(async move {
            conn.accept().await;
        });

        // check request path
        if request.uri().path() != self.path {
            let _ = response.send_response(
                Response::builder()
                    .status(StatusCode::NOT_FOUND)
                    .body(())
                    .unwrap(),
                true,
            );
            return Err(utils::new_io_err("invalid path"));
        }

        // get recv stream from request body
        let (_, recv) = request.into_parts();

        /*
        // prepare server push
        let mut pushed_uri_parts: Parts = parts.uri.into();
        pushed_uri_parts.path_and_query =
            PathAndQuery::from_static("/push").into();
        let push = response.push_request(
            Request::builder()
                .uri(Uri::from_parts(pushed_uri_parts).unwrap())
                .body(())
                .unwrap(),
        );
        */

        // respond a send stream
        let send = response
            .send_response(
                Response::builder().status(StatusCode::OK).body(()).unwrap(),
                false,
            )
            .map_err(|e| utils::new_io_err(&e.to_string()))?;

        /*
        // try server push
        if self.server_push {
            if let Ok(mut push) = push {
                let send = push.send_response(Response::new(()), false);
                if let Ok(send) = send {
                    return Ok((
                        H2Stream::new(
                            recv,
                            send,
                            BytesMut::with_capacity(H2_BUF_SIZE),
                        ),
                        addr,
                    ));
                }
            }
        }
        */

        // fallback
        Ok((
            H2Stream::new(recv, send, BytesMut::with_capacity(H2_BUF_SIZE)),
            addr,
        ))
    }
}

use std::io;
use std::cmp::min;
use std::pin::Pin;
use std::task::{Poll, Context};
use std::net::SocketAddr;
use std::sync::{Arc, RwLock};
use std::sync::atomic::{AtomicUsize, Ordering};
use futures::ready;

use bytes::{Bytes, BytesMut};
use http::{Uri, Version, StatusCode, Request, Response};
use tokio::io::{AsyncRead, AsyncWrite};
use h2::{SendStream, RecvStream};
use h2::client::{self, SendRequest};
use h2::server::{self, SendResponse};

use async_trait::async_trait;

use super::{AsyncConnect, AsyncAccept, IOStream, Transport};
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
                    .map_err(|e| utils::new_io_err(&e.to_string()))
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
pub struct Connector<T: AsyncConnect> {
    cc: T,
    uri: Uri,
    allow_push: bool,
    max_concurrent: usize,
    count: AtomicUsize,
    channel: RwLock<Option<SendRequest<Bytes>>>,
}

impl<T: AsyncConnect> Connector<T> {
    pub fn new(
        cc: T,
        path: String,
        allow_push: bool,
        max_concurrent: usize,
    ) -> Self {
        let authority = cc.addr().to_string();
        let max_concurrent = if max_concurrent == 0 {
            1000
        } else {
            max_concurrent
        };
        Connector {
            cc,
            uri: Uri::builder()
                .scheme(Self::SCHEME)
                .authority(authority.as_str())
                .path_and_query(path)
                .build()
                .unwrap(),
            allow_push,
            max_concurrent,
            count: AtomicUsize::new(1),
            channel: RwLock::new(None),
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

    #[inline]
    fn addr(&self) -> &CommonAddr { self.cc.addr() }

    #[inline]
    async fn connect(&self) -> io::Result<Self::IO> {
        let mut client = new_client(self).await?;

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

async fn new_client<T: AsyncConnect>(
    cc: &Connector<T>,
) -> io::Result<SendRequest<Bytes>> {
    // reuse existed connection
    let channel = (*cc.channel.read().unwrap()).clone();
    if let Some(channel) = channel {
        if cc.count.load(Ordering::Relaxed) < cc.max_concurrent {
            if let Ok(client) = channel.ready().await {
                cc.count.fetch_add(1, Ordering::Relaxed);
                return Ok(client);
            };
        };
    };

    // establish a new connection
    let stream = cc.cc.connect().await?;
    let (client, conn) = client::Builder::new()
        .enable_push(cc.allow_push)
        .handshake(stream)
        .await
        .map_err(|e| utils::new_io_err(&e.to_string()))?;

    // store connection
    // may have conflicts
    cc.count.store(1, Ordering::Relaxed);
    *cc.channel.write().unwrap() = Some(client.clone());
    tokio::spawn(conn);

    client
        .ready()
        .await
        .map_err(|e| utils::new_io_err(&e.to_string()))
}

// HTTP2 Acceptor
pub struct Acceptor<L: AsyncAccept, C> {
    cc: Arc<C>,
    lis: L,
    path: String,
    #[allow(dead_code)]
    server_push: bool,
}

impl<L, C> Acceptor<L, C>
where
    L: AsyncAccept,
{
    pub fn new(cc: Arc<C>, lis: L, path: String, server_push: bool) -> Self {
        Acceptor {
            cc,
            lis,
            path,
            server_push,
        }
    }
}

// Single Connection
#[async_trait]
impl<L> AsyncAccept for Acceptor<L, ()>
where
    L: AsyncAccept,
{
    const TRANS: Transport = Transport::H2;

    const SCHEME: &'static str = match L::TRANS {
        Transport::TLS => "https",
        _ => "http",
    };

    type IO = H2Stream;

    type Base = L::Base;

    #[inline]
    fn addr(&self) -> &CommonAddr { self.lis.addr() }

    #[inline]
    async fn accept_base(&self) -> io::Result<(Self::Base, SocketAddr)> {
        self.lis.accept_base().await
    }

    #[inline]
    async fn accept(&self, base: Self::Base) -> io::Result<Self::IO> {
        let stream = self.lis.accept(base).await?;
        // establish a new connection
        let mut conn = server::handshake(stream)
            .await
            .map_err(|e| utils::new_io_err(&e.to_string()))?;

        // accept a new stream
        let (request, response) = conn
            .accept()
            .await
            .unwrap()
            .map_err(|e| utils::new_io_err(&e.to_string()))?;

        // handle the initial request
        let h2_stream = handle_request(&self.path, request, response).await?;

        Ok(h2_stream)
    }
}

// Mux
#[async_trait]
impl<L, C> AsyncAccept for Acceptor<L, C>
where
    L: AsyncAccept,
    C: AsyncConnect + 'static,
{
    const TRANS: Transport = Transport::H2;

    const SCHEME: &'static str = match L::TRANS {
        Transport::TLS => "https",
        _ => "http",
    };

    type IO = H2Stream;

    type Base = L::Base;

    #[inline]
    fn addr(&self) -> &CommonAddr { self.lis.addr() }

    #[inline]
    async fn accept_base(&self) -> io::Result<(Self::Base, SocketAddr)> {
        self.lis.accept_base().await
    }

    #[inline]
    async fn accept(&self, base: Self::Base) -> io::Result<Self::IO> {
        let stream = self.lis.accept(base).await?;
        // establish a new connection
        let mut conn = server::handshake(stream)
            .await
            .map_err(|e| utils::new_io_err(&e.to_string()))?;

        // accept a new stream
        let (request, response) = conn
            .accept()
            .await
            .unwrap()
            .map_err(|e| utils::new_io_err(&e.to_string()))?;

        // handle the initial request
        let h2_stream = handle_request(&self.path, request, response).await?;

        // handle next mux requests
        tokio::spawn(handle_mux_conn(self.cc.clone(), conn, self.path.clone()));

        Ok(h2_stream)
    }
}

async fn handle_request(
    path: &str,
    request: Request<RecvStream>,
    mut response: SendResponse<Bytes>,
) -> io::Result<H2Stream> {
    // check request path
    if request.uri().path() != path {
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
    Ok(H2Stream::new(
        recv,
        send,
        BytesMut::with_capacity(H2_BUF_SIZE),
    ))
}

async fn handle_mux_conn<C, IO>(
    cc: Arc<C>,
    mut conn: server::Connection<IO, Bytes>,
    path: String,
) where
    C: AsyncConnect + 'static,
    IO: AsyncRead + AsyncWrite + Unpin,
{
    use crate::io::bidi_copy_with_stream;
    while let Some(Ok((request, response))) = conn.accept().await {
        if let Ok(stream) = handle_request(&path, request, response).await {
            tokio::spawn(bidi_copy_with_stream(cc.clone(), stream));
        }
    }
}

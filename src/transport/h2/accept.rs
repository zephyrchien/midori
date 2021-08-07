use std::net::SocketAddr;
use std::io::{Result, Error, ErrorKind};
use std::sync::Arc;

use log::{debug, info, warn};
use async_trait::async_trait;
use bytes::Bytes;
use http::{StatusCode, Request, Response};
use tokio::io::{AsyncRead, AsyncWrite};
use h2::RecvStream;
use h2::server::{self, SendResponse};

use super::H2Stream;
use crate::utils::CommonAddr;
use crate::transport::{AsyncConnect, AsyncAccept, Transport};

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
        Transport::TLS => "h2",
        _ => "h2c",
    };

    type IO = H2Stream;

    type Base = L::Base;

    #[inline]
    fn addr(&self) -> &CommonAddr { self.lis.addr() }

    #[inline]
    async fn accept_base(&self) -> Result<(Self::Base, SocketAddr)> {
        self.lis.accept_base().await
    }

    async fn accept(&self, base: Self::Base) -> Result<Self::IO> {
        let stream = self.lis.accept(base).await?;
        debug!("h2 accept[new] <-");
        // establish a new connection
        let mut conn = server::handshake(stream)
            .await
            .map_err(|e| Error::new(ErrorKind::ConnectionAborted, e))?;

        // accept a new stream
        let (request, response) = conn
            .accept()
            .await
            .unwrap()
            .map_err(|e| Error::new(ErrorKind::Interrupted, e))?;

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
        Transport::TLS => "h2",
        _ => "h2c",
    };

    type IO = H2Stream;

    type Base = L::Base;

    #[inline]
    fn addr(&self) -> &CommonAddr { self.lis.addr() }

    #[inline]
    async fn accept_base(&self) -> Result<(Self::Base, SocketAddr)> {
        self.lis.accept_base().await
    }

    async fn accept(&self, base: Self::Base) -> Result<Self::IO> {
        let stream = self.lis.accept(base).await?;
        debug!("h2 accept[new] <-");
        // establish a new connection
        let mut conn = server::handshake(stream)
            .await
            .map_err(|e| Error::new(ErrorKind::ConnectionAborted, e))?;

        // accept a new stream
        let (request, response) = conn
            .accept()
            .await
            .unwrap()
            .map_err(|e| Error::new(ErrorKind::Interrupted, e))?;

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
) -> Result<H2Stream> {
    // check request path
    if request.uri().path() != path {
        debug!("check request path -- not found");
        let _ = response.send_response(
            Response::builder()
                .status(StatusCode::NOT_FOUND)
                .body(())
                .unwrap(),
            true,
        );
        return Err(Error::new(ErrorKind::NotFound, "invalid path"));
    }
    debug!("check request path -- ok");

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
        .map_err(|e| Error::new(ErrorKind::Interrupted, e))?;

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
    Ok(H2Stream::new(recv, send))
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
    loop {
        match conn.accept().await {
            Some(x) => match x {
                Ok((request, response)) => handle_request(
                    &path, request, response,
                )
                .await
                .map_or_else(
                    |e| debug!("failed to resolve h2-mux stream, {}", e),
                    |stream| {
                        info!(
                            "new h2 stream[reuse] <-> {}[{}]",
                            cc.addr(),
                            C::SCHEME
                        );
                        tokio::spawn(bidi_copy_with_stream(cc.clone(), stream));
                    },
                ),
                Err(e) => {
                    warn!("failed to recv h2-mux response, {}", e);
                    return;
                }
            },
            None => warn!("no more h2-mux stream"),
        }
    }
    /*
    while let Some(Ok((request, response))) = conn.accept().await {

        if let Ok(stream) = handle_request(&path, request, response).await {
            tokio::spawn(bidi_copy_with_stream(cc.clone(), stream));
        }
    }*/
}

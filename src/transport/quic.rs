use std::pin::Pin;
use std::task::{Poll, Context};
use std::io::{Result, Error, ErrorKind};
use std::net::SocketAddr;
use std::sync::{Arc, RwLock};
use std::sync::atomic::{AtomicUsize, Ordering};
use futures::StreamExt;

use log::{warn, info, debug, trace};
use async_trait::async_trait;
use tokio::io::{AsyncRead, AsyncWrite};
use quinn::crypto::rustls::TlsSession;
use quinn::generic::{SendStream, RecvStream, Connection};
use quinn::{Endpoint, NewConnection, Incoming, IncomingBiStreams};

use super::{AsyncConnect, AsyncAccept, IOStream, Transport};
use crate::dns;
use crate::utils::{self, CommonAddr};

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

// Connector
pub struct Connector {
    cc: Endpoint,
    addr: CommonAddr,
    sni: String,
    max_concurrent: usize,
    count: AtomicUsize,
    channel: RwLock<Option<Connection<TlsSession>>>,
}

impl Connector {
    pub fn new(
        cc: Endpoint,
        addr: CommonAddr,
        sni: String,
        max_concurrent: usize,
    ) -> Self {
        let max_concurrent = if max_concurrent == 0 || max_concurrent > 100 {
            100
        } else {
            max_concurrent
        };
        Connector {
            cc,
            addr,
            sni,
            max_concurrent,
            count: AtomicUsize::new(1),
            channel: RwLock::new(None),
        }
    }
}

#[async_trait]
impl AsyncConnect for Connector {
    const TRANS: Transport = Transport::QUIC;

    const SCHEME: &'static str = "quic";

    type IO = QuicStream;

    #[inline]
    fn addr(&self) -> &CommonAddr { &self.addr }

    #[inline]
    fn clear_reuse(&self) { *self.channel.write().unwrap() = None; }

    async fn connect(&self) -> Result<Self::IO> {
        let client = new_client(self).await?;
        let (send, recv) = client.open_bi().await?;
        Ok(QuicStream::new(send, recv))
    }
}

async fn new_client(cc: &Connector) -> Result<Connection<TlsSession>> {
    // reuse existed connection
    trace!("quic init new client");
    let channel = (*cc.channel.read().unwrap()).clone();
    if let Some(client) = channel {
        let count = cc.count.load(Ordering::Relaxed);
        trace!("quic reusable, current mux = {}", count);
        if count < cc.max_concurrent {
            debug!("quic connect[reuse {}] -> {}", count, &cc.addr);
            cc.count.fetch_add(1, Ordering::Relaxed);
            return Ok(client);
        };
    };

    // establish a new connection
    let connect_addr = match &cc.addr {
        CommonAddr::SocketAddr(sockaddr) => *sockaddr,
        CommonAddr::DomainName(addr, port) => {
            let ip = dns::resolve_async(addr).await?;
            SocketAddr::new(ip, *port)
        }
        #[cfg(all(unix, feature = "uds"))]
        CommonAddr::UnixSocketPath(_) => unreachable!(),
    };

    debug!("quic connect[new] -> {}", &cc.addr);
    let connecting = cc
        .cc
        .connect(&connect_addr, &cc.sni)
        .map_err(|e| Error::new(ErrorKind::ConnectionRefused, e))?;

    // early data
    let new_conn = match connecting.into_0rtt() {
        Ok((new_conn, zero_rtt)) => {
            zero_rtt.await;
            new_conn
        }
        Err(connecting) => connecting.await?,
    };

    let NewConnection {
        connection: client, ..
    } = new_conn;

    // store connection
    // may have conflicts
    cc.count.store(1, Ordering::Relaxed);
    *cc.channel.write().unwrap() = Some(client.clone());
    Ok(client)
}

// Acceptor
pub struct Acceptor<C> {
    cc: Arc<C>,
    lis: Incoming,
    addr: CommonAddr,
}

impl<C> Acceptor<C> {
    pub fn new(cc: Arc<C>, lis: Incoming, addr: CommonAddr) -> Self {
        Acceptor { cc, lis, addr }
    }
}

// Single Connection
#[async_trait]
impl AsyncAccept for Acceptor<()> {
    const TRANS: Transport = Transport::QUIC;

    const SCHEME: &'static str = "quic";

    type IO = QuicStream;

    type Base = QuicStream;

    fn addr(&self) -> &CommonAddr { &self.addr }

    async fn accept_base(&self) -> Result<(Self::Base, SocketAddr)> {
        // new connection
        let lis = unsafe { utils::const_cast(&self.lis) };
        let connecting = lis.next().await.ok_or_else(|| {
            Error::new(ErrorKind::ConnectionAborted, "connection abort")
        })?;

        // early data
        let new_conn = match connecting.into_0rtt() {
            Ok((new_conn, _)) => new_conn,
            Err(connecting) => connecting.await?,
        };

        let NewConnection {
            connection: x,
            mut bi_streams,
            ..
        } = new_conn;

        debug!("quic accept[new] <- {}", &x.remote_address());

        let (send, recv) = bi_streams.next().await.ok_or_else(|| {
            Error::new(ErrorKind::Interrupted, "no more stream")
        })??;

        Ok((QuicStream::new(send, recv), x.remote_address()))
    }

    async fn accept(&self, base: Self::Base) -> Result<Self::IO> { Ok(base) }
}

// Mux
#[async_trait]
impl<C> AsyncAccept for Acceptor<C>
where
    C: AsyncConnect + 'static,
{
    const TRANS: Transport = Transport::QUIC;

    const SCHEME: &'static str = "quic";

    type IO = QuicStream;

    type Base = QuicStream;

    fn addr(&self) -> &CommonAddr { &self.addr }

    async fn accept_base(&self) -> Result<(Self::Base, SocketAddr)> {
        // new connection
        let lis = unsafe { utils::const_cast(&self.lis) };
        let connecting = lis.next().await.ok_or_else(|| {
            Error::new(ErrorKind::ConnectionAborted, "connection abort")
        })?;

        // early data
        let new_conn = match connecting.into_0rtt() {
            Ok((new_conn, _)) => new_conn,
            Err(connecting) => connecting.await?,
        };

        let NewConnection {
            connection: x,
            mut bi_streams,
            ..
        } = new_conn;

        debug!("quic accept[new] <- {}", &x.remote_address());

        let (send, recv) = bi_streams.next().await.ok_or_else(|| {
            Error::new(ErrorKind::Interrupted, "no more stream")
        })??;

        tokio::spawn(handle_mux_conn(self.cc.clone(), bi_streams));
        Ok((QuicStream::new(send, recv), x.remote_address()))
    }

    async fn accept(&self, base: Self::Base) -> Result<Self::IO> { Ok(base) }
}

async fn handle_mux_conn<C>(cc: Arc<C>, mut bi_streams: IncomingBiStreams)
where
    C: AsyncConnect + 'static,
{
    use crate::io::bidi_copy_with_stream;

    loop {
        match bi_streams.next().await {
            Some(x) => match x {
                Ok((send, recv)) => {
                    info!(
                        "new quic stream[reuse] <-> {}[{}]",
                        cc.addr(),
                        C::SCHEME
                    );
                    tokio::spawn(bidi_copy_with_stream(
                        cc.clone(),
                        QuicStream::new(send, recv),
                    ));
                }
                Err(e) => {
                    warn!("failed to resolve quic-mux stream, {}", e);
                    return;
                }
            },
            None => warn!("no more quic-mux stream"),
        }
    }
    /*
    while let Some(Ok((send, recv))) = bi_streams.next().await {
        tokio::spawn(bidi_copy_with_stream(
            cc.clone(),
            QuicStream::new(send, recv),
        ));
    }
    */
}

// Raw Acceptor, used to setup the Quic Acceptor above
pub struct RawAcceptor {
    lis: Incoming,
    addr: CommonAddr,
}

impl RawAcceptor {
    pub fn new(lis: Incoming, addr: CommonAddr) -> Self {
        RawAcceptor { lis, addr }
    }
    pub fn set_connector<C>(self, cc: Arc<C>) -> Acceptor<C> {
        Acceptor::new(cc, self.lis, self.addr)
    }
}

#[async_trait]
impl AsyncAccept for RawAcceptor {
    const TRANS: Transport = Transport::QUIC;

    const SCHEME: &'static str = "quic";

    type IO = QuicStream;

    type Base = QuicStream;

    fn addr(&self) -> &CommonAddr { &self.addr }

    async fn accept_base(&self) -> Result<(Self::Base, SocketAddr)> {
        // new connection
        let lis = unsafe { utils::const_cast(&self.lis) };
        let connecting = lis.next().await.ok_or_else(|| {
            Error::new(ErrorKind::ConnectionAborted, "connection abort")
        })?;

        // early data
        let new_conn = match connecting.into_0rtt() {
            Ok((new_conn, _)) => new_conn,
            Err(connecting) => connecting.await?,
        };

        let NewConnection {
            connection: x,
            mut bi_streams,
            ..
        } = new_conn;

        let (send, recv) = bi_streams.next().await.ok_or_else(|| {
            Error::new(ErrorKind::Interrupted, "no more stream")
        })??;

        Ok((QuicStream::new(send, recv), x.remote_address()))
    }

    async fn accept(&self, base: Self::Base) -> Result<Self::IO> { Ok(base) }
}

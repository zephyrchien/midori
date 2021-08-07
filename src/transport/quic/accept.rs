use std::io::{Result, Error, ErrorKind};
use std::net::SocketAddr;
use std::sync::Arc;
use futures::StreamExt;

use log::{warn, info, debug};
use async_trait::async_trait;

use quinn::{NewConnection, Incoming, IncomingBiStreams};

use super::QuicStream;
use crate::utils::{self, CommonAddr};
use crate::transport::{AsyncConnect, AsyncAccept, Transport};

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

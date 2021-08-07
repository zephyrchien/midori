use std::io;
use std::net::SocketAddr;
use async_trait::async_trait;
use tokio::io::{AsyncRead, AsyncWrite};
use crate::utils::CommonAddr;

pub mod plain;

#[cfg(feature = "ws")]
pub mod ws;

#[cfg(feature = "h2c")]
pub mod h2;

#[cfg(feature = "tls")]
pub mod tls;

#[cfg(feature = "udp")]
pub mod udp;

#[cfg(feature = "quic")]
pub mod quic;

trait IOStream: AsyncRead + AsyncWrite + Send + Sync + Unpin {}

#[allow(clippy::upper_case_acronyms)]
pub enum Transport {
    TCP,
    TLS,
    WS,
    H2,
    UDP,
    QUIC,
}

#[async_trait]
pub trait AsyncConnect: Send + Sync + Unpin {
    const TRANS: Transport;
    const SCHEME: &'static str;
    type IO: AsyncRead + AsyncWrite + Send + Sync + Unpin + 'static;
    fn addr(&self) -> &CommonAddr;
    async fn connect(&self) -> io::Result<Self::IO>;
}

#[async_trait]
pub trait AsyncAccept: Send + Sync + Unpin {
    const TRANS: Transport;
    const SCHEME: &'static str;
    type IO: AsyncRead + AsyncWrite + Send + Sync + Unpin + 'static;
    type Base: AsyncRead + AsyncWrite + Send + Sync + Unpin + 'static;
    fn addr(&self) -> &CommonAddr;
    // initial accept
    async fn accept_base(&self) -> io::Result<(Self::Base, SocketAddr)>;
    // protocol handshake
    async fn accept(&self, base: Self::Base) -> io::Result<Self::IO>;
}

use std::io;
use std::net::SocketAddr;
use async_trait::async_trait;
use tokio::io::{AsyncRead, AsyncWrite};
use crate::utils::CommonAddr;

pub mod plain;
pub mod ws;
pub mod h2;
pub mod tls;
use plain::PlainStream;

trait IOStream: AsyncRead + AsyncWrite + Send + Sync + Unpin {}

#[allow(clippy::upper_case_acronyms)]
pub enum Transport {
    TCP,
    TLS,
    WS,
    H2,
}

#[async_trait]
pub trait AsyncConnect: Send + Sync + Unpin + Clone {
    const TRANS: Transport;
    const SCHEME: &'static str;
    type IO: AsyncRead + AsyncWrite + Send + Sync + Unpin + 'static;
    fn addr(&self) -> &CommonAddr;
    async fn connect(&self) -> io::Result<Self::IO>;
}

#[async_trait]
pub trait AsyncAccept: Send + Sync + Unpin + Clone {
    const TRANS: Transport;
    const SCHEME: &'static str;
    const MUX: bool;
    type IO: AsyncRead + AsyncWrite + Send + Sync + Unpin + 'static;
    fn addr(&self) -> &CommonAddr;
    async fn accept(
        &self,
        // this is only used by the initial accept
        res: (PlainStream, SocketAddr),
    ) -> io::Result<(Self::IO, SocketAddr)>;
}

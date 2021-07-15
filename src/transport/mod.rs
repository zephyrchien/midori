use std::io;
use std::net::SocketAddr;
use async_trait::async_trait;
use tokio::io::{AsyncRead, AsyncWrite};

pub mod plain;
pub mod ws;

trait IOStream: AsyncRead + AsyncWrite + Send + Sync + Unpin {}

#[async_trait]
pub trait AsyncConnect: Send + Sync + Unpin {
    //const is_zero_copy:bool;
    type IO: AsyncRead + AsyncWrite + Send + Sync + Unpin + 'static;
    async fn connect(&self) -> io::Result<Self::IO>;
}

#[async_trait]
pub trait AsyncAccept: Send + Sync + Unpin {
    //const is_zero_copy:bool;
    type IO: AsyncRead + AsyncWrite + Send + Sync + Unpin + 'static;
    async fn accept(&self) -> io::Result<(Self::IO, SocketAddr)>;
}

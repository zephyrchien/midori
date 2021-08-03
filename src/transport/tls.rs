use std::io::Result;
use std::net::SocketAddr;
use std::sync::Arc;

use async_trait::async_trait;
use webpki::DNSName;
use rustls::{ClientConfig, ServerConfig};
use tokio_rustls::{TlsAcceptor, TlsConnector};
// re-export
pub use tokio_rustls::client::TlsStream as ClientTLSStream;
pub use tokio_rustls::server::TlsStream as ServerTLSStream;

use super::{AsyncConnect, AsyncAccept, Transport};
use crate::utils::{self, CommonAddr};

pub struct Connector<T: AsyncConnect> {
    cc: T,
    sni: DNSName,
    // includes inner tls config
    tls: TlsConnector,
}

impl<T: AsyncConnect> Connector<T> {
    pub fn new(cc: T, sni: DNSName, tlsc: ClientConfig) -> Self {
        Self {
            cc,
            sni,
            tls: TlsConnector::from(Arc::new(tlsc)),
        }
    }
}

#[async_trait]
impl<T: AsyncConnect> AsyncConnect for Connector<T> {
    const TRANS: Transport = Transport::TLS;

    const SCHEME: &'static str = "tls";

    type IO = ClientTLSStream<T::IO>;

    #[inline]
    fn addr(&self) -> &CommonAddr { self.cc.addr() }

    #[inline]
    async fn connect(&self) -> Result<Self::IO> {
        let stream = self.cc.connect().await?;
        self.tls.connect(self.sni.as_ref(), stream).await
    }
}

pub struct Acceptor<T: AsyncAccept> {
    lis: T,
    // includes inner tls config
    tls: TlsAcceptor,
}

impl<T: AsyncAccept> Acceptor<T> {
    pub fn new(lis: T, tlsc: ServerConfig) -> Self {
        Self {
            lis,
            tls: TlsAcceptor::from(Arc::new(tlsc)),
        }
    }
}

#[async_trait]
impl<T: AsyncAccept> AsyncAccept for Acceptor<T> {
    const TRANS: Transport = Transport::TLS;

    const SCHEME: &'static str = "tls";

    type IO = ServerTLSStream<T::IO>;

    type Base = T::Base;

    #[inline]
    fn addr(&self) -> &utils::CommonAddr { self.lis.addr() }

    #[inline]
    async fn accept_base(&self) -> Result<(Self::Base, SocketAddr)> {
        self.lis.accept_base().await
    }

    #[inline]
    async fn accept(&self, base: Self::Base) -> Result<Self::IO> {
        let stream = self.lis.accept(base).await?;
        Ok(self.tls.accept(stream).await?)
    }
}

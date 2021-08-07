use std::sync::Arc;
use std::fmt::{Display, Formatter};
use serde::{Serialize, Deserialize};

use super::WithTransport;
use crate::transport::{AsyncConnect, AsyncAccept};

#[allow(clippy::upper_case_acronyms)]
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "proto", rename_all = "lowercase")]
pub enum TransportConfig {
    Plain,
    #[cfg(feature = "ws")]
    WS(WebSocketConfig),
    #[cfg(feature = "h2c")]
    H2(HTTP2Config),
    #[cfg(feature = "quic")]
    QUIC(QuicConfig),
}

impl Default for TransportConfig {
    fn default() -> Self { Self::Plain }
}

impl Display for TransportConfig {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        use TransportConfig::*;
        match self {
            Plain => write!(f, "raw"),
            #[cfg(feature = "ws")]
            WS(_) => write!(f, "ws"),
            #[cfg(feature = "h2c")]
            H2(_) => write!(f, "h2c"),
            #[cfg(feature = "quic")]
            QUIC(_) => write!(f, "quic"),
        }
    }
}

// ===== Details =====
#[cfg(feature = "ws")]
#[derive(Debug, Serialize, Deserialize)]
pub struct WebSocketConfig {
    pub path: String,
}

#[cfg(feature = "h2c")]
#[derive(Debug, Serialize, Deserialize)]
pub struct HTTP2Config {
    pub path: String,

    #[serde(default)]
    pub server_push: bool,
    #[serde(default)]
    pub mux: usize,
}

#[cfg(feature = "quic")]
#[derive(Debug, Serialize, Deserialize)]
pub struct QuicConfig {
    #[serde(default)]
    pub mux: usize,
}

// ===== Loaders =====
#[cfg(feature = "ws")]
use crate::transport::ws;
#[cfg(feature = "ws")]
impl<L, C> WithTransport<L, C> for WebSocketConfig
where
    L: AsyncAccept,
    C: AsyncConnect,
{
    type Acceptor = ws::Acceptor<L>;
    type Connector = ws::Connector<C>;

    fn apply_to_lis(&self, lis: L) -> Self::Acceptor {
        ws::Acceptor::new(lis, self.path.clone())
    }

    fn apply_to_conn(&self, conn: C) -> Self::Connector {
        ws::Connector::new(conn, self.path.clone())
    }

    fn apply_to_lis_with_conn(&self, _: Arc<C>, _: L) -> Self::Acceptor {
        unreachable!()
    }
}

#[cfg(feature = "h2c")]
use crate::transport::h2;
#[cfg(feature = "h2c")]
impl<L, C> WithTransport<L, C> for HTTP2Config
where
    L: AsyncAccept,
    C: AsyncConnect + 'static,
{
    type Acceptor = h2::Acceptor<L, C>;
    type Connector = h2::Connector<C>;

    fn apply_to_lis(&self, _: L) -> Self::Acceptor { unreachable!() }

    fn apply_to_conn(&self, conn: C) -> Self::Connector {
        h2::Connector::new(conn, self.path.clone(), self.server_push, self.mux)
    }

    fn apply_to_lis_with_conn(&self, conn: Arc<C>, lis: L) -> Self::Acceptor {
        h2::Acceptor::new(conn, lis, self.path.clone(), self.server_push)
    }
}

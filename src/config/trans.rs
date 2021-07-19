use serde::{Serialize, Deserialize};

use super::WithTransport;
use crate::transport::{ws, h2};
use crate::transport::{AsyncConnect, AsyncAccept};

#[allow(clippy::upper_case_acronyms)]
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "proto", rename_all = "lowercase")]
pub enum TransportConfig {
    Plain,
    WS(WebSocketConfig),
    H2(HTTP2Config),
}

#[derive(Debug, Serialize, Deserialize)]
pub struct WebSocketConfig {
    pub path: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct HTTP2Config {
    pub path: String,
}

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
}

impl<L, C> WithTransport<L, C> for HTTP2Config
where
    L: AsyncAccept,
    C: AsyncConnect,
{
    type Acceptor = h2::Acceptor<L>;
    type Connector = h2::Connector<C>;

    fn apply_to_lis(&self, lis: L) -> Self::Acceptor {
        h2::Acceptor::new(lis, self.path.clone())
    }
    fn apply_to_conn(&self, conn: C) -> Self::Connector {
        h2::Connector::new(conn, self.path.clone())
    }
}

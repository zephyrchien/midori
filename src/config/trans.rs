use serde::{Serialize, Deserialize};

use crate::transport::{plain, ws};
use crate::transport::{AsyncConnect, AsyncAccept};

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "proto", rename_all = "lowercase")]
pub enum TransportConfig {
    Plain,
    WS(WebSocketConfig),
}

#[derive(Debug, Serialize, Deserialize)]
pub struct WebSocketConfig {
    pub path: String,
}

pub trait WithTransport<L = plain::Acceptor, C = plain::Connector>
where
    L: AsyncAccept,
    C: AsyncConnect,
{
    type Acceptor: AsyncAccept;
    type Connector: AsyncConnect;

    fn apply_to_lis(&self, lis: L) -> Self::Acceptor;
    fn apply_to_conn(&self, conn: C) -> Self::Connector;
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

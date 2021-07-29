use std::fs;
use std::sync::Arc;
use serde::{Serialize, Deserialize};
use crate::transport::plain;
use crate::transport::{AsyncConnect, AsyncAccept};

mod dns;
mod ep;
mod net;
mod tls;
mod trans;
// re-export
pub use dns::DnsMode;
pub use net::NetConfig;
pub use tls::{TLSConfig, TLSClientConfig, TLSServerConfig};
pub use trans::{TransportConfig, WebSocketConfig, HTTP2Config};
pub use ep::{EndpointConfig, EpHalfConfig, MaybeHalfConfig};

#[derive(Debug, Serialize, Deserialize)]
pub struct GlobalConfig {
    #[serde(default)]
    pub dns_mode: DnsMode,
    pub endpoints: Vec<EndpointConfig>,
}

impl GlobalConfig {
    pub fn from_config_file(file: &str) -> Self {
        let config = fs::read_to_string(file).expect("invalid file path");
        serde_json::from_str(&config).expect("failed to parse config file")
    }
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
    fn apply_to_lis_with_conn(&self, conn: Arc<C>, lis: L) -> Self::Acceptor;
}

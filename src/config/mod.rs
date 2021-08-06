use std::fs;
use std::sync::Arc;
use serde::{Serialize, Deserialize};

use crate::utils::must;
use crate::transport::{AsyncConnect, AsyncAccept};

pub mod dns;
pub mod ep;
pub mod net;
pub mod tls;
pub mod trans;
// re-export
pub use dns::DnsMode;
pub use net::NetConfig;
pub use tls::TLSConfig;
pub use trans::TransportConfig;
pub use ep::{EndpointConfig, EpHalfConfig, MaybeHalfConfig};

#[derive(Debug, Serialize, Deserialize)]
pub struct GlobalConfig {
    #[serde(default)]
    pub dns_mode: DnsMode,
    pub endpoints: Vec<EndpointConfig>,
}

impl GlobalConfig {
    pub fn from_config_file(file: &str) -> Self {
        let config = must!(fs::read_to_string(file), "load {}", file);
        must!(serde_json::from_str(&config), "parse json")
    }
}

pub trait WithTransport<L, C>
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

use std::fs;
use serde::{Serialize, Deserialize};

mod dns;
mod ep;
mod net;
mod trans;

pub use dns::DnsMode;
pub use net::NetConfig;
pub use trans::{WithTransport, TransportConfig, WebSocketConfig};
pub use ep::EndpointConfig;

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

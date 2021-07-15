use serde::{Serialize, Deserialize};

use super::net::NetConfig;
use super::trans::TransportConfig;

#[derive(Debug, Serialize, Deserialize)]
pub struct EpHalfConfig {
    pub addr: String,
    pub net: NetConfig,
    pub trans: TransportConfig,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EndpointConfig {
    pub listen: EpHalfConfig,
    pub remote: EpHalfConfig,
}

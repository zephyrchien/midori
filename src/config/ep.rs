use serde::{Serialize, Deserialize};

use super::net::NetConfig;
use super::tls::TLSConfig;
use super::trans::TransportConfig;

#[derive(Debug, Serialize, Deserialize)]
pub struct EpHalfConfig {
    pub addr: String,
    #[serde(default = "def_net")]
    pub net: NetConfig,
    #[serde(default = "def_trans")]
    pub trans: TransportConfig,
    #[serde(default = "def_tls")]
    pub tls: TLSConfig,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EndpointConfig {
    pub listen: EpHalfConfig,
    pub remote: EpHalfConfig,
}

// create default values
fn def_net() -> NetConfig { NetConfig::TCP }

fn def_trans() -> TransportConfig { TransportConfig::Plain }

fn def_tls() -> TLSConfig { TLSConfig::None }

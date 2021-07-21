use serde::{Serialize, Deserialize};

use super::net::NetConfig;
use super::tls::TLSConfig;
use super::trans::TransportConfig;

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct EpHalfConfig {
    pub addr: String,

    #[serde(default)]
    pub net: NetConfig,

    #[serde(default)]
    pub trans: TransportConfig,

    #[serde(default)]
    pub tls: TLSConfig,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum MaybeHalfConfig {
    Addr(String),
    Config(EpHalfConfig),
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EndpointConfig {
    pub listen: MaybeHalfConfig,
    pub remote: MaybeHalfConfig,
}

impl From<MaybeHalfConfig> for EpHalfConfig {
    fn from(x: MaybeHalfConfig) -> Self {
        match x {
            MaybeHalfConfig::Addr(s) => EpHalfConfig {
                addr: s,
                ..Default::default()
            },
            MaybeHalfConfig::Config(c) => c,
        }
    }
}

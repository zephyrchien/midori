use std::fmt::{Display, Formatter};
use serde::{Serialize, Deserialize};

#[allow(clippy::upper_case_acronyms)]
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum NetConfig {
    TCP,
    UDP,
    UDS,
}

impl Default for NetConfig {
    fn default() -> Self { Self::TCP }
}

impl Display for NetConfig {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        use NetConfig::*;
        match self {
            TCP => write!(f, "tcp"),
            UDP => write!(f, "udp"),
            UDS => write!(f, "uds"),
        }
    }
}

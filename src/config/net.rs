use std::fmt::{Display, Formatter};
use serde::{Serialize, Deserialize};

#[allow(clippy::upper_case_acronyms)]
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum NetConfig {
    TCP,
    #[cfg(feature = "udp")]
    UDP,
    #[cfg(all(unix, feature = "uds"))]
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

            #[cfg(feature = "udp")]
            UDP => write!(f, "udp"),

            #[cfg(all(unix, feature = "uds"))]
            UDS => write!(f, "uds"),
        }
    }
}

impl NetConfig {
    #[cfg(target_os = "linux")]
    pub fn is_zero_copy(&self) -> bool {
        use NetConfig::*;
        match self {
            TCP => true,
            #[cfg(feature = "udp")]
            UDP => false,
            #[cfg(feature = "uds")]
            UDS => true,
        }
    }
}

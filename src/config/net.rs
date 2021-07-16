use serde::{Serialize, Deserialize};

#[allow(clippy::upper_case_acronyms)]
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum NetConfig {
    TCP,
    UDP,
    #[cfg(unix)]
    UDS,
}

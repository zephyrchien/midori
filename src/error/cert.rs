use std::fmt::{Display, Formatter};
use std::error::Error;

pub type Result<T> = std::result::Result<T, CertError>;

#[derive(Debug)]
pub enum CertError {
    LoadCertificate,
    LoadPrivateKey,
    GenCertKey(rcgen::RcgenError),
    IO(std::io::Error),
}

impl Display for CertError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        use CertError::*;
        match self {
            LoadCertificate => write!(f, "failed to load certificate"),
            LoadPrivateKey => write!(f, "failed to load private key"),
            GenCertKey(..) => {
                write!(f, "failed to generate certificate key pair")
            }
            IO(..) => write!(f, "failed to open file"),
        }
    }
}

impl Error for CertError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        use CertError::*;
        match self {
            LoadCertificate => None,
            LoadPrivateKey => None,
            GenCertKey(e) => Some(e),
            IO(e) => Some(e),
        }
    }
}

impl From<rcgen::RcgenError> for CertError {
    fn from(e: rcgen::RcgenError) -> Self { CertError::GenCertKey(e) }
}

impl From<std::io::Error> for CertError {
    fn from(e: std::io::Error) -> Self { CertError::IO(e) }
}

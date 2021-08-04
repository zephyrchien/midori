use std::fmt::{Display, Formatter};
use std::error::Error;

pub type Result<T> = std::result::Result<T, AddrError>;

#[derive(Debug)]
pub enum AddrError {
    Invalid(String),
    IO(std::io::Error),
}

impl Display for AddrError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        use AddrError::*;
        match self {
            Invalid(s) => write!(f, "invalid address: {}", s),
            IO(..) => write!(f, "failed to open file"),
        }
    }
}

impl Error for AddrError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        use AddrError::*;
        match self {
            Invalid(_) => None,
            IO(e) => Some(e),
        }
    }
}

impl From<std::io::Error> for AddrError {
    fn from(e: std::io::Error) -> Self { AddrError::IO(e) }
}

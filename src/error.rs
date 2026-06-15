use std::net::AddrParseError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum MkvPeelError {
    #[error("unexpected: {0}")]
    Unexpected(&'static str),

    #[error("parse: {0}")]
    Parse(#[from] AddrParseError),
    
    #[error("packet: {0}, operation: {1}")]
    Packet(&'static str, &'static str),
    
    #[error("io: {0}")]
    Io(#[from] std::io::Error),

    #[error("nul: {0}")]
    Nul(#[from] std::ffi::NulError),
}

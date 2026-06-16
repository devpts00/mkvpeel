use std::net::AddrParseError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum MkvPeelError {
    #[error("parse: {0}")]
    Parse(#[from] AddrParseError),

    #[error("io: {0}")]
    Io(#[from] std::io::Error),

    #[error("mkv: {0}")]
    Mkv(#[from] matroska_demuxer::DemuxError),

    #[error("nul: {0}")]
    Nul(#[from] std::ffi::NulError),
}

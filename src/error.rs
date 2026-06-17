use std::path::PathBuf;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum MkvPeelError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),

    #[error("mkv: {0}")]
    Mkv(#[from] matroska_demuxer::DemuxError),

    #[error("utf8: {0}")]
    Utf8(#[from] std::str::Utf8Error),

    #[error("nul: {0}")]
    Nul(#[from] std::ffi::NulError),

    #[error("file name: {0}")]
    FileName(PathBuf),
    
    #[error("format: {0}")]
    Format(#[from] std::fmt::Error),
}

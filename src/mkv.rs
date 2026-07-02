use std::ffi::OsStr;
use std::path::Path;
use regex::Regex;
use crate::error::MkvPeelError;
use crate::peel::{MkvPeel, TrackBuff};

pub struct Mkv;

impl MkvPeel for Mkv {
    fn probe(&self, path: &Path) -> Result<bool, MkvPeelError> {
        let meta = path.metadata()?;
        Ok(meta.is_file() && path.extension().map(|ext| ext.eq_ignore_ascii_case(OsStr::new("mkv"))).unwrap_or(false))
    }
    fn peel(&self, src: &Path, dst: &Path, languages: &[Regex], buffs: &[TrackBuff]) -> Result<(), MkvPeelError> {
        todo!()
    }
}

use std::fmt::Display;
use std::path::Path;
use regex::Regex;
use crate::error::MkvPeelError;

#[derive(Debug, Clone)]
pub enum TrackKind {
    Audio,
    Subtitles,
}

impl Display for TrackKind {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            TrackKind::Audio => write!(f, "audio"),
            TrackKind::Subtitles => write!(f, "subtitles"),
        }
    }
}

#[derive(Debug, Clone)]
pub enum TrackField {
    Codec,
    Name
}

#[derive(Debug, Clone)]
pub struct TrackBuff {
    pub kind: TrackKind,
    pub field: TrackField,
    pub regex: Regex,
    pub value: i16,
}

impl TrackBuff {
    pub fn new(kind: TrackKind, field: TrackField, regex: Regex, value: i16) -> Self {
        Self { kind, field, regex, value }
    }
}

pub trait MkvPeel {
    fn probe(&self, path: &Path) -> Result<bool, MkvPeelError>;
    fn peel(&self, src: &Path, dst: &Path, languages: &[Regex], buffs: &[TrackBuff]) -> Result<(), MkvPeelError>;
}

use std::fmt::{Display, Formatter};
use std::num::ParseIntError;
use std::str::FromStr;
use clap::Parser;
use clap_duration::duration_range_value_parse;
use duration_human::{DurationHuman, DurationHumanValidator};
use matroska_demuxer::TrackType;
use regex::{Regex, RegexBuilder};
use thiserror::Error;

#[derive(Debug, Error)]
pub struct InvalidTokenError {
    kind: & 'static str,
    token: String
}

impl InvalidTokenError {
    fn new(kind: &'static str, token: &str) -> Self {
        let token = token.to_string();
        Self { kind, token }
    }
}

impl Display for InvalidTokenError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "invalid {}: '{}'", self.kind, self.token)
    }
}

#[derive(Debug, Clone)]
pub struct TrackKind(pub TrackType);

impl Display for TrackKind {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self.0 {
            TrackType::Unknown => write!(f, "unknown"),
            TrackType::Video => write!(f, "video"),
            TrackType::Audio => write!(f, "audio"),
            TrackType::Complex => write!(f, "complex"),
            TrackType::Logo => write!(f, "logo"),
            TrackType::Subtitle => write!(f, "subtitle"),
            TrackType::Buttons => write!(f, "buttons"),
            TrackType::Control => write!(f, "control"),
            TrackType::Metadata => write!(f, "metadata"),
        }
    }
}

impl FromStr for TrackKind {
    type Err = InvalidTokenError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "a" => Ok(Self(TrackType::Audio)),
            "s" => Ok(Self(TrackType::Subtitle)),
            t => Err(InvalidTokenError::new("track", t))
        }
    }
}

#[derive(Debug, Clone)]
pub enum TrackField {
    Codec,
    Name
}

impl FromStr for TrackField {
    type Err = InvalidTokenError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "c" => Ok(Self::Codec),
            "n" => Ok(Self::Name),
            f => Err(InvalidTokenError::new("field", f))
        }
    }
}

#[derive(Debug, Error)]
pub enum InvalidBuffError {
    #[error("format")]
    Format,
    #[error("token: {0}")]
    Token(#[from] InvalidTokenError),
    #[error("parse: {0}")]
    Score(#[from] ParseIntError),
    #[error("regex: {0}")]
    Regex(#[from] regex::Error),
}

#[derive(Debug, Clone)]
pub struct Buff {
    pub kind: TrackKind,
    pub field: TrackField,
    pub regex: Regex,
    pub value: i16,
}

impl Buff {
    fn new(kind: TrackKind, field: TrackField, regex: Regex, value: i16) -> Self {
        Self { kind, field, regex, value }
    }
}

impl FromStr for Buff {
    type Err = InvalidBuffError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut split = s.split(":").into_iter();
        let track: TrackKind = split.next().ok_or(InvalidBuffError::Format)?.parse()?;
        let field: TrackField = split.next().ok_or(InvalidBuffError::Format)?.parse()?;
        let regex = split.next().ok_or(InvalidBuffError::Format)?;
        let regex = RegexBuilder::new(regex).case_insensitive(true).build()?;
        let score: i16 = split.next().ok_or(InvalidBuffError::Format)?.parse()?;
        Ok(Buff::new(track, field, regex, score))
    }
}

#[derive(Parser, Debug)]
#[command(version, about)]
pub struct Cmd {
    #[arg(long, short)]
    pub src: String,
    #[arg(long, short)]
    pub dst: String,
    #[arg(long, value_delimiter = ',')]
    pub languages: Vec<String>,
    #[arg(long, short)]
    pub buff: Vec<Buff>,
    #[arg(long, default_value = "60s", value_parser = duration_range_value_parse!(min: 10s, max: 10min))]
    pub pause: DurationHuman,
    #[arg(long, default_value = "60s", value_parser = duration_range_value_parse!(min: 10s, max: 60min))]
    pub age: DurationHuman,
}

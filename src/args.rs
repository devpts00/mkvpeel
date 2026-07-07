use std::num::ParseIntError;
use std::str::FromStr;
use clap::Parser;
use clap_duration::duration_range_value_parse;
use duration_human::{DurationHuman, DurationHumanValidator};
use isolang::Language;
use regex::{Regex, RegexBuilder};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum InvalidBuffError {
    #[error("format")]
    Format,
    #[error("parse: {0}")]
    Buff(#[from] ParseIntError),
    #[error("regex: {0}")]
    Regex(#[from] regex::Error),
}


#[derive(Debug, Clone)]
pub struct TrackBuff {
    pub regex: Regex,
    pub value: i16,
}

impl TrackBuff {
    pub fn new(regex: Regex, value: i16) -> Self {
        Self { regex, value }
    }
}

impl FromStr for TrackBuff {
    type Err = InvalidBuffError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut split = s.split(":").into_iter();
        let regex = split.next().ok_or(InvalidBuffError::Format)?;
        let regex = RegexBuilder::new(regex).case_insensitive(true).build()?;
        let score: i16 = split.next().ok_or(InvalidBuffError::Format)?.parse()?;
        Ok(TrackBuff::new(regex, score))
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
    pub languages: Vec<Language>,
    #[arg(long)]
    pub codec: Vec<TrackBuff>,
    #[arg(long)]
    pub name: Vec<TrackBuff>,
    #[arg(long, default_value = "60s", value_parser = duration_range_value_parse!(min: 10s, max: 10min))]
    pub pause: DurationHuman,
    #[arg(long, default_value = "60s", value_parser = duration_range_value_parse!(min: 10s, max: 60min))]
    pub age: DurationHuman,
}

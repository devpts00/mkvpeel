use std::collections::HashMap;
use std::fmt::{Display, Formatter};
use std::path::Path;
use isolang::Language;
use regex::Regex;
use tracing::{debug, info};
use crate::error::MkvPeelError;
use crate::util::write_opt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

#[derive(Debug, Clone, Copy)]
pub enum TrackField {
    Codec,
    Name
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

pub struct TrackDisplay<'a, T: Track> {
    inner: &'a T
}

impl <'a, T: Track> From<&'a T> for TrackDisplay<'a, T> {
    fn from(value: &'a T) -> Self {
        Self { inner: value }
    }
}

impl <'a, T: Track> Display for TrackDisplay<'a, T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "num: ")?;
        write_opt(f, self.inner.number())?;
        write!(f, ", kind: ")?;
        write_opt(f, self.inner.kind())?;
        write!(f, ", codec: ")?;
        write_opt(f, self.inner.field(TrackField::Codec))?;
        write!(f, ", name: ")?;
        write_opt(f, self.inner.field(TrackField::Name))?;
        Ok(())
    }
}

pub trait Track: Sized {
    fn number(&self) -> Option<u16>;
    fn kind(&self) -> Option<TrackKind>;
    fn lang(&self) -> Option<Language>;
    fn field(&self, field: TrackField) -> Option<&str>;
    fn display(&self) -> TrackDisplay<'_, Self> {
        self.into()
    }
}

#[derive(Debug, Clone, Copy)]
pub struct TrackNum {
    num: u16,
    buff: i16,
}

impl TrackNum {
    pub fn new(num: u16, buff: i16) -> Self {
        Self { num, buff }
    }
}

impl Display for TrackNum {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "num: {}, buff: {}", self.num, self.buff)
    }
}

#[inline]
fn buff<T: Track>(track: &T, buffs: &[TrackBuff]) -> i16 {
    let mut sum: i16 = 0;
    if let Some(kind) = track.kind() {
        for buff in buffs {
            // if let Some(value) = track.field(buff.field) {
            //     if buff.regex.is_match(value) {
            //         sum += buff.value
            //     }
            // }
        }
    }
    sum
}

#[inline]
fn collect_ids(tracks: &HashMap<Language, TrackNum>, ids: &mut Vec<u16>) {
    ids.clear();
    for (_, tn) in tracks {
        info!("collect, number: {}, buff: {}", tn.num, tn.buff);
        ids.push(tn.num);
    }
}

#[inline]
fn upsert(tns: &mut HashMap<Language, TrackNum>, lang: Language, tn: TrackNum) {
    match tns.get_mut(&lang) {
        Some(t) => {
            if t.buff < tn.buff {
                debug!("replace, number: {}, buff: {}", tn.num, tn.buff);
                *t = tn
            }
        }
        None => {
            debug!("insert, number: {}, buff: {}", tn.num, tn.buff);
            tns.insert(lang, tn);
        }
    }
}

pub fn collect_track_ids<T: Track>(
    tracks: &[T],
    langs: &[Language],
    buffs: &[TrackBuff],
    audios: &mut HashMap<Language, TrackNum>,
    subtitles: &mut HashMap<Language, TrackNum>,
    audio_ids: &mut Vec<u16>,
    subtitle_ids: &mut Vec<u16>,
) {
    audios.clear();
    subtitles.clear();
    audio_ids.clear();
    subtitle_ids.clear();
    for (idx, track) in tracks.iter().enumerate() {
        debug!("found, idx: {}, track: {}", idx, track.display());
        if let Some(kind) = track.kind() {
            if let Some(lang) = track.lang() {
                if langs.contains(&lang) {
                    let num = track.number().unwrap_or(idx as u16);
                    let buff = buff(track, buffs);
                    let tn = TrackNum::new(num, buff);
                    match kind {
                        TrackKind::Audio => {
                            upsert(audios, lang, tn);
                        }
                        TrackKind::Subtitles => {
                            upsert(subtitles, lang, tn);
                        }
                    }
                }
            }
        }
    }
    collect_ids(audios, audio_ids);
    collect_ids(subtitles, audio_ids);
}

pub trait MkvPeel {
    fn probe(&self, path: &Path) -> Result<bool, MkvPeelError>;
    fn peel(&self, src: &Path, dst: &Path, langs: &[Language], buffs: &[TrackBuff]) -> Result<(), MkvPeelError>;
}

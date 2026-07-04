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
struct TrackNum {
    num: u16,
    buff: i16,
}

impl TrackNum {
    fn new(num: u16, buff: i16) -> Self {
        Self { num, buff }
    }
}

impl Display for TrackNum {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "num: {}, buff: {}", self.num, self.buff)
    }
}


#[inline]
fn check_lang(lang: Language, langs: &[Language]) -> bool {
    langs.contains(&lang)
}

#[inline]
fn buff<T: Track>(track: &T, buffs: &[TrackBuff]) -> i16 {
    let mut sum: i16 = 0;
    if let Some(kind) = track.kind() {
        for buff in buffs {
            if buff.kind == kind {
                if let Some(value) = track.field(buff.field) {
                    if buff.regex.is_match(value) {
                        sum += buff.value
                    }
                }
            }
        }
    }
    sum
}

#[inline]
fn collect_ids(tracks: HashMap<Language, TrackNum>) -> Vec<u16> {
    tracks.into_iter().map(|(_, tn)| {
        info!("collect, number: {}, buff: {}", tn.num, tn.buff);
        tn.num
    }).collect()
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

pub fn tracks<T: Track>(tracks: &[T], langs: &[Language], buffs: &[TrackBuff]) -> (Vec<u16>, Vec<u16>) {
    let mut audios: HashMap<Language, TrackNum> = HashMap::new();
    let mut subtitles: HashMap<Language, TrackNum> = HashMap::new();
    for (idx, track) in tracks.iter().enumerate() {
        debug!("found, idx: {}, track: {}", idx, track.display());
        if let Some(kind) = track.kind() {
            if let Some(lang) = track.lang() {
                if check_lang(lang, langs) {
                    let num = track.number().unwrap_or(idx as u16);
                    let buff = buff(track, buffs);
                    let tn = TrackNum::new(num, buff);
                    match kind {
                        TrackKind::Audio => {
                            upsert(&mut audios, lang, tn);
                        }
                        TrackKind::Subtitles => {
                            upsert(&mut subtitles, lang, tn);
                        }
                    }
                }
            }
        }
    }
    (collect_ids(audios), collect_ids(subtitles))
}

pub trait MkvPeel {
    fn probe(&self, path: &Path) -> Result<bool, MkvPeelError>;
    fn peel(&self, src: &Path, dst: &Path, langs: &[Language], buffs: &[TrackBuff]) -> Result<(), MkvPeelError>;
}

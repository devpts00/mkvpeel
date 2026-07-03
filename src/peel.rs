use std::collections::HashMap;
use std::fmt::{Display, Formatter};
use std::path::Path;
use regex::Regex;
use tracing::{debug, info};
use crate::error::MkvPeelError;

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

pub trait Track {
    fn number(&self) -> u16;
    fn kind(&self) -> Option<TrackKind>;
    fn lang(&self) -> Option<&str>;
    fn field(&self, field: TrackField) -> Option<&str>;
}

struct TrackBuffed<'a, T> {
    track: &'a T,
    buff: i16,
}

impl <'a, T: Track> TrackBuffed<'a, T> {
    fn new(track: &'a T) -> Self {
        Self { track, buff: 0 }
    }
    fn buff(&mut self, buffs: &[TrackBuff]) {
        if let Some(kind) = self.track.kind() {
            for buff in buffs {
                if buff.kind == kind {
                    if let Some(value) = self.track.field(buff.field) {
                        if buff.regex.is_match(value) {
                            self.buff += buff.value
                        }
                    }
                }
            }
        }
    }
}

impl <'a, T: Track> Display for TrackBuffed<'a, T> {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        write!(f, "number: {}, ", self.track.number())?;
        match self.track.kind() {
            Some(kind) => write!(f, "kind: {}, ", kind)?,
            None => write!(f, "kind: und, ")?,
        };
        write!(f, "lang: {}, ", self.track.lang().unwrap_or("und"))?;
        write!(f, "codec: {}, ", self.track.field(TrackField::Codec).unwrap_or("und"))?;
        write!(f, "name: {}, ", self.track.field(TrackField::Name).unwrap_or("und"))?;
        write!(f, "buff: {}", self.buff)?;
        Ok(())
    }
}

#[inline]
fn check_language(language: &str, languages: &[Regex]) -> bool {
    languages.iter().any(|r| r.is_match_at(language, 0))
}

#[inline]
fn collect_ids<T: Track>(tracks: HashMap<&str, TrackBuffed<T>>) -> Vec<u16> {
    tracks.into_iter().map(|(_, tb)| {
        info!("collect, {}", tb);
        tb.track.number()
    }).collect()
}

#[inline]
fn modify_or_insert2<'a, 'b, T: Track>(tracks: &'b mut HashMap<&'a str, TrackBuffed<'a, T>>, language: &'a str, track: TrackBuffed<'a, T>) {
    match tracks.get_mut(language) {
        Some(t) => {
            if t.buff < track.buff {
                debug!("replace, {}", track);
                *t = track
            }
        }
        None => {
            debug!("insert, {}", track);
            tracks.insert(language, track);
        }
    }
}

pub fn tracks<T: Track>(tracks: &[T], langs: &[Regex], buffs: &[TrackBuff]) -> (Vec<u16>, Vec<u16>) {
    let mut audios: HashMap<&str, TrackBuffed<T>> = HashMap::new();
    let mut subtitles: HashMap<&str, TrackBuffed<T>> = HashMap::new();
    for track in tracks {
        let mut tb = TrackBuffed::new(track);
        debug!("found, {}", tb);
        if let Some(kind) = tb.track.kind() {
            if let Some(lang) = tb.track.lang() {
                if check_language(lang, langs) {
                    tb.buff(buffs);
                    match kind {
                        TrackKind::Audio => {
                            modify_or_insert2(&mut audios, lang, tb);
                        }
                        TrackKind::Subtitles => {
                            modify_or_insert2(&mut subtitles, lang, tb);
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
    fn peel(&self, src: &Path, dst: &Path, languages: &[Regex], buffs: &[TrackBuff]) -> Result<(), MkvPeelError>;
}

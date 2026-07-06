use std::collections::HashMap;
use std::ffi::OsStr;
use std::fmt::{Display, Formatter, Write};
use std::io::{ErrorKind};
use std::mem::swap;
use std::path::{Path, PathBuf};
use std::process::{Command};
use isolang::Language;
use tracing::{debug, warn};
use crate::error::MkvPeelError;
use crate::model::{PlaylistInfo, TrackInfo};
use crate::peel::{collect_track_ids, MkvPeel, TrackBuff};
use crate::util::{join, pipe, ToOption};

fn buff_all(value: Option<&str>, buffs: &[TrackBuff]) -> i16 {
    let mut buff = 0;
    if let Some(v) = value {
        for b in buffs {
            if b.regex.is_match(v) {
                buff += b.value;
            }
        }
    }
    buff
}

#[derive(Debug, Clone, Copy)]
pub struct TrackCtx {
    id: u16,
    buff: i16,
}

impl TrackCtx {
    fn new(id: u16, buff: i16) -> Self {
        Self { id, buff }
    }
}

impl Display for TrackCtx {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "id: {}, buff: {}", self.id, self.buff)
    }
}

struct TrackKindCtx {
    tracks: HashMap<Language, TrackCtx>,
    buf: String,
}

impl TrackKindCtx {
    fn new() -> Self {
        Self { tracks: HashMap::with_capacity(16), buf: String::with_capacity(64) }
    }
    fn clear(&mut self) {
        self.tracks.clear();
        self.buf.clear();
    }
    fn upsert(&mut self, langs: &[Language], codecs: &[TrackBuff], names: &[TrackBuff], track: &TrackInfo) {
        if let Some(lang) = track.lang() {
            if langs.contains(&lang) {
                let id = track.id();
                let buff = buff_all(track.codec(), codecs) + buff_all(track.name(), names);
                let ctx = TrackCtx::new(id, buff);
                match self.tracks.get_mut(&lang) {
                    Some(t) => {
                        if t.buff < ctx.buff {
                            debug!("replace, id: {}, buff: {}", ctx.id, ctx.buff);
                            *t = ctx
                        }
                    }
                    None => {
                        debug!("insert, id: {}, buff: {}", ctx.id, ctx.buff);
                        self.tracks.insert(lang, ctx);
                    }
                }
            }
        }
    }
    fn is_empty(&self) -> bool {
        self.tracks.is_empty()
    }
    fn ids(&mut self) -> Result<&str, std::fmt::Error> {
        if self.buf.is_empty() {
            for (_, track) in &self.tracks {
                write!(&mut self.buf, "{},", track.id)?;
            }
            if !self.buf.is_empty() {
                self.buf.truncate(self.buf.len() - 1)
            }
        }
        Ok(self.buf.as_str())
    }
}

struct PlayListCtx {
    path: PathBuf,
    audio: TrackKindCtx,
    subtitles: TrackKindCtx,
    score: u16
}

impl PlayListCtx {
    fn new() -> Self {
        Self { path: PathBuf::new(), audio: TrackKindCtx::new(), subtitles: TrackKindCtx::new(), score: 0 }
    }
    fn clear(&mut self) {
        self.path.clear();
        self.audio.clear();
        self.subtitles.clear();
        self.score = 0;
    }
    fn reload(&mut self, path: PathBuf, playlist_info: &PlaylistInfo, langs: &[Language], codecs: &[TrackBuff], names: &[TrackBuff]) {
        self.clear();
        self.path = path;
        for track_info in playlist_info.tracks() {
            if let Some(kind) = track_info.kind() {
                match kind {
                    "video" => {
                        self.score += 1000;
                    }
                    "audio" => {
                        self.score += 100;
                        self.audio.upsert(langs, codecs, names, track_info);
                    }
                    "subtitles" => {
                        self.score += 10;
                        self.subtitles.upsert(langs, codecs, names, track_info);
                    }
                    _ => {
                    }
                }
            }
        }
    }
}

pub struct JsonImpl {
    langs: Vec<Language>,
    codecs: Vec<TrackBuff>,
    names: Vec<TrackBuff>,
    max: PlayListCtx,
    cur: PlayListCtx,
    buf: String,
    bdmv: &'static OsStr,
    extensions: Vec<&'static OsStr>,
}

impl JsonImpl {
    pub fn new(langs: Vec<Language>, codecs: Vec<TrackBuff>, names: Vec<TrackBuff>) -> Self {
        Self {
            langs,
            codecs,
            names,
            max: PlayListCtx::new(),
            cur: PlayListCtx::new(),
            buf: String::with_capacity(4 * 1024),
            bdmv: OsStr::new("BDMV"),
            extensions: vec![OsStr::new("mkv"), OsStr::new("mp4"), OsStr::new("avi"), OsStr::new("mov")],
        }
    }
    pub fn check(&self, path: &Path) -> Result<bool, MkvPeelError> {
        let meta = path.metadata()?;
        if meta.is_dir() {
            match path.join(self.bdmv).metadata() {
                Ok(meta) => {
                    Ok(meta.is_dir())
                },
                Err(err) => {
                    if err.kind() == ErrorKind::NotFound {
                        Ok(false)
                    } else {
                        Err(MkvPeelError::Io(err))
                    }
                }
            }
        } else {
            Ok(path.extension()
                .map(|ext| self.extensions.iter().any(|e| e.eq_ignore_ascii_case(ext)))
                .unwrap_or(false))
        }
    }
    fn peel(&mut self, src: PathBuf, dst: &Path) -> Result<(), MkvPeelError> {
        let meta = src.metadata()?;
        if meta.is_dir() {
            let dir = src.join("BDMV/PLAYLIST");
            let entries = dir.read_dir()?;
            for entry in entries {
                let entry = entry?;
                let meta = entry.metadata()?;
                if meta.is_file() {
                    let src = entry.path();
                    if let Some(ext) = src.extension() {
                        if ext == OsStr::new("mpls") {
                            let info = PlaylistInfo::load(&src, &mut self.buf)?;
                            self.cur.reload(src, &info, &self.langs, &self.codecs, &self.names);
                            if self.max.score < self.cur.score {
                                swap(&mut self.max, &mut self.cur);
                            }
                        }
                    }
                }
            }
        } else {
            let info = PlaylistInfo::load(&src, &mut self.buf)?;
            self.max.reload(src, &info, &self.langs, &self.codecs, &self.names);
        }
        let mut mkvmerge = Command::new("mkvmerge");
        mkvmerge.arg("--verbose");
        mkvmerge.arg("--output").arg(dst);
        if !self.max.audio.is_empty() {
            let ids = self.max.audio.ids()?;
            mkvmerge.arg("--audio-tracks").arg(ids);
        }
        if !self.max.subtitles.is_empty() {
            let ids = self.max.subtitles.ids()?;
            mkvmerge.arg("--subtitle-tracks").arg(ids);
        }
        mkvmerge.arg(&self.max.path);
        debug!("run: {:?}", mkvmerge);
        pipe(mkvmerge)?;
        Ok(())
    }
}


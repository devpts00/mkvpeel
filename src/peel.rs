use std::borrow::Cow;
use std::collections::HashMap;
use std::ffi::OsStr;
use std::fmt::{Display, Formatter, Write};
use std::io::{ErrorKind};
use std::mem::swap;
use std::path::{Path};
use std::process::{Command};
use std::time::Duration;
use isolang::Language;
use tracing::{debug, info};
use crate::args::TrackBuff;
use crate::error::MkvPeelError;
use crate::json::{PlaylistInfo, TrackInfo};
use crate::util::{pipe};

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
    fn get_ids(&mut self) -> Result<&str, std::fmt::Error> {
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
    audio: TrackKindCtx,
    subtitles: TrackKindCtx,
    score: u16
}

impl PlayListCtx {
    fn new() -> Self {
        Self { audio: TrackKindCtx::new(), subtitles: TrackKindCtx::new(), score: 0 }
    }
    fn clear(&mut self) {
        self.audio.clear();
        self.subtitles.clear();
        self.score = 0;
    }
    fn reload(&mut self, playlist_info: &PlaylistInfo, langs: &[Language], codecs: &[TrackBuff], names: &[TrackBuff], skip_commentary: bool) {
        self.clear();
        for track_info in playlist_info.tracks() {
            if !track_info.is_commentary() || !skip_commentary {
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
}

pub struct PeelCtx {
    langs: Vec<Language>,
    codecs: Vec<TrackBuff>,
    names: Vec<TrackBuff>,
    skip_commentary: bool,
    max: PlayListCtx,
    cur: PlayListCtx,
    buf: String,
    bdmv: &'static OsStr,
    extensions: Vec<&'static OsStr>,
}

impl PeelCtx {
    pub fn new(langs: Vec<Language>, codecs: Vec<TrackBuff>, names: Vec<TrackBuff>, skip_commentary: bool) -> Self {
        Self {
            langs,
            codecs,
            names,
            skip_commentary,
            max: PlayListCtx::new(),
            cur: PlayListCtx::new(),
            buf: String::with_capacity(32 * 1024),
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
    pub fn peel(&mut self, src: &Path, dst: &Path) -> Result<(), MkvPeelError> {
        self.max.clear();
        self.cur.clear();
        debug!("peel, src: {}, dst: {}", src.display(), dst.display());
        let meta = src.metadata()?;
        let mut src_max: Option<Cow<Path>> = None;
        if meta.is_dir() {
            let dir = src.join("BDMV/PLAYLIST");
            let entries = dir.read_dir()?;
            for entry in entries {
                let entry = entry?;
                let meta = entry.metadata()?;
                if meta.is_file() {
                    let src_cur = entry.path();
                    if let Some(ext) = src_cur.extension() {
                        if ext == OsStr::new("mpls") {
                            let info = PlaylistInfo::load(&src_cur, &mut self.buf)?;
                            if info.recognized() && info.supported() {
                                if let Some(duration) = info.duration() {
                                    if Duration::from_hours(1) <= duration && duration <= Duration::from_hours(6) {
                                        self.cur.reload(&info, &self.langs, &self.codecs, &self.names, self.skip_commentary);
                                        //info!("reload, current: {}, score: {}", src_cur.display(), self.cur.score);
                                        if self.max.score < self.cur.score {
                                            debug!("lead: {}, score: {}", src_cur.display(), self.cur.score);
                                            swap(&mut self.max, &mut self.cur);
                                            src_max = Some(Cow::Owned(src_cur));
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        } else {
            let info = PlaylistInfo::load(src, &mut self.buf)?;
            self.max.reload(&info, &self.langs, &self.codecs, &self.names, self.skip_commentary);
            src_max = Some(Cow::Borrowed(src))
        }

        if let Some(src_max) = src_max {
            let mut mkvmerge = Command::new("mkvmerge");
            mkvmerge.arg("--output").arg(dst);
            mkvmerge.arg("--no-buttons");
            mkvmerge.arg("--no-attachments");
            mkvmerge.arg("--no-global-tags");
            if !self.max.audio.is_empty() {
                let ids = self.max.audio.get_ids()?;
                mkvmerge.arg("--audio-tracks").arg(ids);
            }
            if !self.max.subtitles.is_empty() {
                let ids = self.max.subtitles.get_ids()?;
                mkvmerge.arg("--subtitle-tracks").arg(ids);
            }
            mkvmerge.arg(src_max.as_ref());
            info!("run: {:?}", mkvmerge);
            pipe(mkvmerge)?;
        }
        Ok(())
    }
}


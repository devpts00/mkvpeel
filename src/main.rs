use std::collections::HashMap;
use std::ffi::OsStr;
use std::fmt::Write;
use std::fs::{metadata, read_dir, rename, File};
use std::io::{Read, Seek};
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::str::{from_utf8, Utf8Error};
use std::thread::sleep;
use std::time::Duration;
use chrono::{Datelike, Utc};
use clap::Parser;
use matroska_demuxer::{MatroskaFile, TrackEntry, TrackType};
use regex::Regex;
use thiserror::__private18::AsDisplay;
use tracing::{debug, error, info, trace, warn};
use crate::args::{Buff, Cmd, TrackField};
use crate::bdmv::Bdmv;
use crate::common::{age, extract_name_without_ext, get_min_age, make_pretty_name, opt_get_age_ext};
use crate::error::MkvPeelError;
use crate::mkv::Mkv;
use crate::peel::{MkvPeel, MkvProbe, TrackBuff};
use crate::util::{init_tracing, join, log, ok_warn, ToOption};

mod util;
mod args;
mod error;
pub mod bdmv;
pub mod mkv;
pub mod common;
pub mod peel;

#[inline]
fn get_value<'a>(t: &'a TrackEntry, f: &TrackField) -> Option<&'a str> {
    match f {
        TrackField::Codec => Some(t.codec_id()),
        TrackField::Name => t.name(),
    }
}

#[inline]
fn buff_one(e: &TrackEntry, b: &Buff, buff: &mut i16) {
    if e.track_type() == b.kind.0 {
        if let Some(v) = get_value(e, &b.field) {
            let mut matched = false;
            if b.regex.is_match(v) {
                *buff += b.value;
                matched = true;
            }
            trace!("match, result: {}, regex: {}, value: {}", matched, b.regex, v);
        }
    }
}

#[inline]
fn buff_all<'a, 'b>(e: &'a TrackEntry, bs: &'b [TrackBuff]) -> TrackEntryBuff<'a> {
    let mut buff = 0;
    for b in bs {
        buff_one(&e, &b, &mut buff);
    }
    TrackEntryBuff::new(e, buff)
}

#[inline]
fn extract_track_entry(entry: &TrackEntry) -> (u64, TrackType, &str, &str, &str) {
    let number = entry.track_number().get();
    let kind = entry.track_type();
    let language = entry.language_bcp47().unwrap_or("n/a");
    let codec = entry.codec_id();
    let name = entry.name().unwrap_or("n/a");
    (number, kind, language, codec, name)
}

#[inline]
fn debug_track_entry(verb: &'static str, entry: &TrackEntry, buff: i16) {
    let (number, kind, language, codec, name) = extract_track_entry(entry);
    debug!("{}, track: {}, kind: {:?}, lang: {}, codec: {}, name: {}, buff: {}", verb, number - 1, kind, language, codec, name, buff);
}

#[inline]
fn check_language(language: &str, languages: &[Regex]) -> bool {
    languages.iter().any(|r| {
        r.find(language)
            .map(|m| m.start() == 0)
            .unwrap_or(false)
    })
}

#[inline]
fn collect_ids(tracks: HashMap<&str, TrackEntryBuff>) -> Vec<u64> {
    tracks.into_iter().map(|(_, track)| {
        let (number, kind, language, codec, name) = extract_track_entry(&track.entry);
        info!("{:?}, track: {}, lang: {}, codec: {}, name: {}", kind, number, language, codec, name);
        number - 1
    }).collect()
}

struct TrackEntryBuff<'a> {
    entry: &'a TrackEntry,
    buff: i16
}

impl <'a> TrackEntryBuff<'a> {
    fn new(entry: &'a TrackEntry, buff: i16) -> Self {
        Self { entry, buff }
    }
}

fn modify_or_insert<'a, 'b>(tracks: &'b mut HashMap<&'a str, TrackEntryBuff<'a>>, language: &'a str, track: TrackEntryBuff<'a>) {
    match tracks.get_mut(language) {
        Some(t) => {
            if t.buff < track.buff {
                debug_track_entry("replace", &track.entry, track.buff);
                *t = track
            }
        }
        None => {
            debug_track_entry("insert", &track.entry, track.buff);
            tracks.insert(language, track);
        }
    }
}

fn tracks<R: Read + Seek>(
    mkv: MatroskaFile<R>,
    languages: &[Regex],
    buffs: &[TrackBuff],
) -> (Vec<u64>, Vec<u64>) {
    let mut audios: HashMap<&str, TrackEntryBuff> = HashMap::new();
    let mut subtitles: HashMap<&str, TrackEntryBuff> = HashMap::new();
    for entry in mkv.tracks() {
        debug_track_entry("found", &entry, 0);
        if let Some(language) = entry.language_bcp47() {
            if check_language(language, languages) {
                match entry.track_type() {
                    TrackType::Audio => {
                        modify_or_insert(&mut audios, language, buff_all(entry, &buffs));
                    }
                    TrackType::Subtitle => {
                        modify_or_insert(&mut subtitles, language, buff_all(entry, &buffs));
                    }
                    _ => {
                    }
                }
            }
        }
    }
    let audios = collect_ids(audios);
    let subtitles = collect_ids(subtitles);
    (audios, subtitles)
}

fn run(
    peels: &[Box<dyn MkvPeel>],
    src_dir: &Path,
    dst_dir: &Path,
    languages: &[Regex],
    buff: &[TrackBuff],
    pause: Duration,
    age: Duration
) -> Result<(), MkvPeelError> {
    info!("run, src: {}, dst: {}", src_dir.display(), dst_dir.display());
    loop {
        scan(peels, src_dir, dst_dir, languages, buff, age)?;
        debug!("sleep: {} seconds", pause.as_secs());
        sleep(pause);
    }
}

#[inline]
fn find<'a>(peels: &'a [Box<dyn MkvPeel>], src_path: &Path) -> Option<&'a Box<dyn MkvPeel>> {
    peels.iter().find(|peel|
        peel.probe(src_path).ok_warn("probe").unwrap_or(false)
    )
}

fn scan(
    peels: &[Box<dyn MkvPeel>],
    src_dir: &Path,
    dst_dir: &Path,
    languages: &[Regex],
    buffs: &[TrackBuff],
    min_age: Duration
) -> Result<(), MkvPeelError> {
    for src_dir_entry in read_dir(src_dir)? {
        let src_dir_entry = src_dir_entry?;
        let src_meta = src_dir_entry.metadata()?;
        let src_path = src_dir_entry.path();
        debug!("found: {}", src_path.display());
        match find(peels, &src_path) {
            Some(peel) => {
                if let Some(age) = get_min_age(&src_path, &src_meta).ok_warn("age") {
                    if age >= min_age {
                        match extract_name_without_ext(&src_path, &src_meta) {
                            Some(src_name) => {
                                if let Some(mut dst_name) = make_pretty_name(src_name).ok_warn("prettify") {
                                    dst_name.push_str(".mkv");
                                    let dst_path = dst_dir.join(&dst_name);
                                    peel.peel(&src_path, &dst_path, languages, buffs).ok_warn("peel");
                                }
                            }
                            None => {
                                warn!("name: {}", src_path.as_display());
                            }
                        }
                    }
                }
            }
            None => {
                if src_meta.is_dir() {
                    scan(peels, &src_path, dst_dir, languages, buffs, min_age)?;
                }
            }
        }
    }
    Ok(())
}

fn land(
    src_path: &Path,
    dst_dir: &Path,
    languages: &[Regex],
    buff: &[Buff],
) -> Result<(), MkvPeelError> {
    let src_file = src_path.file_name().ok_or(MkvPeelError::FileName(src_path.to_path_buf()))?;
    let src_file = src_file.as_bytes();
    let src_file = from_utf8(src_file)?;
    let dst_file = rename(src_file)?;
    let dst_path = dst_dir.join(dst_file);
    if !dst_path.exists() {
        peel(src_path, &dst_path, languages, buff)?;
    }
    Ok(())
}


fn peel(
    src_path: &Path,
    dst_path: &Path,
    languages: &[Regex],
    buffs: &[Buff],
) -> Result<(), MkvPeelError> {
    info!("peel, src: '{}', dst: '{}'", src_path.display(), dst_path.display());
    let mut file = File::open(src_path)?;
    match MatroskaFile::open(&mut file) {
        Ok(mkv) => {
            // TODO: consider adding track order
            let (audios, subtitles) = tracks(mkv, languages, buffs);
            let mut mkvmerge = Command::new("mkvmerge");
            mkvmerge.arg("--output").arg(dst_path);
            if !audios.is_empty() {
                mkvmerge.arg("--audio-tracks").arg(join(audios));
            }
            if !subtitles.is_empty() {
                mkvmerge.arg("--subtitle-tracks").arg(join(subtitles));
            }
            mkvmerge.arg(src_path);
            debug!("run: {:?}", mkvmerge);
            mkvmerge.spawn()?.wait()?;
        }
        Err(err) => {
            error!("failed to read mkv file: '{}', probably it is not yet copied, error: {}", src_path.display(), err);
        }
    }
    Ok(())
}

struct Composite {
    peels: Vec<Box<dyn MkvPeel>>,
}

impl Composite {
    fn new() -> Composite {
        Composite {
        }
    }

    fn peel(&self, src_path: &Path, dst_dir: &Path, languages: &[Regex], buffs: &[crate::peel::TrackBuff]) -> Result<(), MkvPeelError> {
        for p in &self.peels {
            match p.probe(src_path) {
                Ok(true) => {
                    let src_file = src_path.file_name().ok_or(MkvPeelError::FileName(src_path.to_path_buf()))?;
                    let src_bytes = src_file.as_bytes();
                    let src_file = from_utf8(src_bytes)?;
                    let src_ext_len = src_path.extension().map(|ext| ext.len()).unwrap_or(0);
                    let src_name = &src_file[..src_file.len() - src_ext_len];
                    let dst_file = make_pretty_name(src_name)?;
                    let dst_path = dst_dir.join(dst_file);
                    if !dst_path.exists() {
                        p.peel(src_path, &dst_path, languages, buffs)?;
                    }
                    break;
                }
                Ok(false) => {
                }
                Err(err) => {
                    warn!("probe: {}", err);
                }
            }
        }
        Ok(())
    }
}

fn main() {
    let _guard = init_tracing();



    let src_dir = PathBuf::from("/home/nomad/Code/mkvpeel/in/PILLOW BOOK_HDCLUB_BY_VOLSHEBNIK/BDMV");
    //log(bdmv(&src_dir));

    let cmd = Cmd::parse();
    debug!("cmd: {:?}", cmd);
    let peels: Vec<Box<dyn MkvPeel>> = vec![Box::new(Mkv), Box::new(Bdmv)];
    let src_dir = Path::new(cmd.src.as_str());
    let dst_dir = Path::new(cmd.dst.as_str());
    let languages = cmd.languages;
    let buff = cmd.buff;
    let pause = Duration::from(&cmd.pause);
    let age = Duration::from(&cmd.age);
    log(run(peels, src_dir, dst_dir, &languages, &buff, pause, age));
}

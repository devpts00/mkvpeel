use std::collections::HashMap;
use std::ffi::OsStr;
use std::fmt::Write;
use std::fs::{metadata, read_dir, File};
use std::io::{Read, Seek};
use std::os::unix::ffi::OsStrExt;
use std::path::Path;
use std::process::Command;
use std::str::from_utf8;
use std::thread::sleep;
use std::time::Duration;
use chrono::{Datelike, Utc};
use clap::Parser;
use matroska_demuxer::{MatroskaFile, TrackEntry, TrackType};
use regex::Regex;
use tracing::{debug, error, info, trace};
use crate::args::{Buff, Cmd, TrackField};
use crate::error::MkvPeelError;
use crate::util::{init_tracing, join, log};

mod util;
mod args;
mod error;

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
fn buff_all<'a, 'b>(e: &'a TrackEntry, bs: &'b [Buff]) -> TrackBuff<'a> {
    let mut buff = 0;
    for b in bs {
        buff_one(&e, &b, &mut buff);
    }
    TrackBuff::new(e, buff)
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
fn collect_ids(tracks: HashMap<&str, TrackBuff>) -> Vec<u64> {
    tracks.into_iter().map(|(_, track)| {
        let (number, kind, language, codec, name) = extract_track_entry(&track.entry);
        info!("{:?}, track: {}, lang: {}, codec: {}, name: {}", kind, number, language, codec, name);
        number - 1
    }).collect()
}

struct TrackBuff<'a> {
    entry: &'a TrackEntry,
    buff: i16
}

impl <'a> TrackBuff<'a> {
    fn new(entry: &'a TrackEntry, buff: i16) -> Self {
        Self { entry, buff }
    }
}

fn modify_or_insert<'a, 'b>(tracks: &'b mut HashMap<&'a str, TrackBuff<'a>>, language: &'a str, track: TrackBuff<'a>) {
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
    buffs: &[Buff],
) -> (Vec<u64>, Vec<u64>) {
    let mut audios: HashMap<&str, TrackBuff> = HashMap::new();
    let mut subtitles: HashMap<&str, TrackBuff> = HashMap::new();
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
    src_dir: &Path,
    dst_dir: &Path,
    languages: &[Regex],
    buff: &[Buff],
    pause: Duration,
    age: Duration
) -> Result<(), MkvPeelError> {
    info!("run, src: {}, dst: {}", src_dir.display(), dst_dir.display());
    let ext_mkv = OsStr::new("mkv");
    loop {
        scan(src_dir, dst_dir, ext_mkv, languages, buff, age)?;
        debug!("sleep: {} seconds", pause.as_secs());
        sleep(pause);
    }
}

fn scan(
    src_dir: &Path,
    dst_dir: &Path,
    ext_mkv: &OsStr,
    languages: &[Regex],
    buff: &[Buff],
    age: Duration
) -> Result<(), MkvPeelError> {
    for src_dir_entry in read_dir(src_dir)? {
        let src_dir_entry = src_dir_entry?;
        let src_path = src_dir_entry.path();
        debug!("found: {}", src_path.display());
        if src_path.is_dir() {
            scan(&src_path, &dst_dir, ext_mkv, languages, buff, age)?;
        } else if let Some(ext) = src_path.extension() {
            if ext == ext_mkv {
                let src_meta = metadata(&src_path)?;
                if src_meta.is_file() {
                    if let Some(modified) = src_meta.modified().ok() {
                        if let Some(elapsed) = modified.elapsed().ok() {
                            if elapsed > age {
                                land(&src_path, dst_dir, languages, buff)?;
                            }
                        }
                    }
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

fn rename(src_mkv: &str) -> Result<String, std::fmt::Error> {
    let src = src_mkv.strip_suffix(".mkv").unwrap_or(src_mkv);
    let mut dst_mkv = String::with_capacity(src.len() + 6);
    let year_now = Utc::now().year() as u64;
    let mut year_unlocked = false;
    let mut year_in_progress = false;
    let mut year_bracketed = false;
    let mut whitespace = false;
    let mut year: u64 = 0;
    for c in src.chars() {
        if '0' <= c && c <= '9' && year_unlocked {
            whitespace = false;
            year_in_progress = true;
            year = 10 * year + (c as u64 - '0' as u64);
        } else {
            if year_in_progress {
                if 1900 <= year && year <= year_now {
                    if !year_bracketed {
                        dst_mkv.push('(');
                    }
                    write!(&mut dst_mkv, "{}", year)?;
                    dst_mkv.push(')');
                    break;
                } else {
                    write!(&mut dst_mkv, "{}", year)?;
                    year_in_progress = false;
                    year = 0;
                }
            }
            year_unlocked = true;
            if c == '.' || c.is_whitespace() {
                year_bracketed = false;
                if !whitespace {
                    dst_mkv.push(' ');
                    whitespace = true;
                }
            } else {
                year_bracketed = c == '(';
                whitespace = false;
                dst_mkv.push(c);
            }
        }
    }
    dst_mkv.push_str(".mkv");
    Ok(dst_mkv)
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

fn main() {
    let _guard = init_tracing();
    let cmd = Cmd::parse();
    debug!("cmd: {:?}", cmd);
    let src_dir = Path::new(cmd.src.as_str());
    let dst_dir = Path::new(cmd.dst.as_str());
    let languages = cmd.languages;
    let buff = cmd.buff;
    let pause = Duration::from(&cmd.pause);
    let age = Duration::from(&cmd.age);
    log(run(src_dir, dst_dir, &languages, &buff, pause, age));
}

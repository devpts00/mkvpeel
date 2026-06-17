use std::cmp::Ordering;
use std::collections::HashMap;
use std::ffi::OsStr;
use std::fmt::{Display, Formatter, Write};
use std::fs::{metadata, read_dir, File};
use std::io::{Read, Seek};
use std::num::NonZeroU64;
use std::os::unix::ffi::OsStrExt;
use std::path::Path;
use std::process::Command;
use std::str::{from_utf8, FromStr};
use std::thread::sleep;
use std::time::Duration;
use chrono::{Datelike, Utc};
use clap::Parser;
use matroska_demuxer::{Audio, MatroskaFile, TrackEntry, TrackType};
use thiserror::Error;
use tracing::{error, info};
use crate::args::{Cmd};
use crate::error::MkvPeelError;
use crate::util::{init_tracing, join, log, to_lowercase};

mod util;
mod args;
mod error;

#[derive(Debug)]
struct TrackKind(TrackType);

impl TrackKind {
    fn new(track_type: TrackType) -> Self { Self(track_type) }
}

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

#[derive(Debug, Error)]
struct UnknownTrackCodecError {
    kind: TrackKind,
    codec: String,
}

impl UnknownTrackCodecError {
    fn new(track_type: TrackType, codec: &str) -> Self {
        UnknownTrackCodecError { kind: TrackKind::new(track_type), codec: codec.to_string() }
    }
}

impl Display for UnknownTrackCodecError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "unknown {} codec: {}", self.kind, self.codec)
    }
}

#[derive(PartialEq, Eq, Ord, PartialOrd)]
enum AudioCodec {
    DTS,
    AC3,
    EAC3,
    TrueHD,
}

impl FromStr for AudioCodec {
    type Err = UnknownTrackCodecError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "A_TRUEHD" => Ok(AudioCodec::TrueHD),
            "A_EAC3" => Ok(AudioCodec::EAC3),
            "A_AC3" => Ok(AudioCodec::AC3),
            "A_DTS" => Ok(AudioCodec::DTS),
            codec => Err(UnknownTrackCodecError::new(TrackType::Audio, codec)),
        }
    }
}

#[derive(PartialEq, Eq, Ord, PartialOrd)]
enum SubtitleCodec {
    PGS,
    SRT,
}

impl FromStr for SubtitleCodec {
    type Err = UnknownTrackCodecError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "S_HDMV/PGS" => Ok(SubtitleCodec::PGS),
            "S_TEXT/UTF8" => Ok(SubtitleCodec::SRT),
            codec => Err(UnknownTrackCodecError::new(TrackType::Subtitle, codec)),
        }
    }
}

struct AudioDetails<'a>(&'a Audio);

impl <'a> AudioDetails<'a> {
    fn new(value: &'a Audio) -> Self {
        AudioDetails(value)
    }
    fn to_tuple(&self) -> (NonZeroU64, Option<NonZeroU64>, f64, Option<f64>) {
        (self.0.channels(), self.0.bit_depth(), self.0.sampling_frequency(), self.0.output_sampling_frequency())
    }
}

impl <'a> From<&'a Audio> for AudioDetails<'a> {
    fn from(value: &'a Audio) -> Self {
        AudioDetails::new(value)
    }
}

impl <'a> PartialEq for AudioDetails<'a> {
    fn eq(&self, other: &Self) -> bool {
        self.to_tuple() == other.to_tuple()
    }
}

impl <'a> Eq for AudioDetails<'a> {}

impl <'a> PartialOrd for AudioDetails<'a> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.to_tuple().partial_cmp(&other.to_tuple())
    }
}

impl <'a> Ord for AudioDetails<'a> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.partial_cmp(other).unwrap_or(Ordering::Equal)
    }
}

struct Track<'a> {
    name: Option<String>,
    entry: &'a TrackEntry,
}

impl <'a> Track<'a> {
    fn new(entry: &'a TrackEntry) -> Self {
        let name = entry.name().map(|n| n.to_lowercase());
        Self { name, entry }
    }
}

#[inline]
fn score_prefer(track: &Track, prefer: &[String]) -> usize {
    match &track.name {
        Some(name) => {
            prefer.iter().enumerate()
                .map(|(i, p)| { if name.contains(p) { i } else { 0 } })
                .sum()
        },
        None => {
            0
        }
    }
}

#[inline]
fn less_audio(t1: &Track, t2: &Track, prefer: &[String]) -> bool {
    let s1 = score_prefer(t1, prefer);
    let s2 = score_prefer(t2, prefer);
    let c1: Option<AudioCodec> = t1.entry.codec_id().parse().ok();
    let c2: Option<AudioCodec> = t2.entry.codec_id().parse().ok();
    let d1: Option<AudioDetails> = t1.entry.audio().map(|a| a.into());
    let d2: Option<AudioDetails> = t2.entry.audio().map(|a| a.into());
    (s1, c1, d1) < (s2, c2, d2)
}

#[inline]
fn less_subtitle(t1: &Track, t2: &Track, prefer: &[String]) -> bool {
    let s1 = score_prefer(t1, prefer);
    let s2 = score_prefer(t1, prefer);
    let c1: Option<SubtitleCodec> = t1.entry.codec_id().parse().ok();
    let c2: Option<SubtitleCodec> = t2.entry.codec_id().parse().ok();
    (s1, c1) < (s2, c2)
}

fn dump(verb: &'static str, track: &TrackEntry) {
    let number = track.track_number().get();
    let language = track.language_bcp47().unwrap_or("n/a");
    let codec = track.codec_id();
    let name = track.name().unwrap_or("n/a");
    let channels = track.audio().map(|a| a.channels());
    info!("{}, track: {}, lang: {}, codec: {}, name: {}, channels: {:?}", verb, number - 1, language, codec, name, channels);
}

#[inline]
fn check_language(language: &str, languages: &[String]) -> bool {
    let language = language.to_lowercase();
    languages.contains(&language)
}

#[inline]
fn check_exclude(name: Option<&str>, exclude: &[String]) -> bool {
    match name {
        Some(name) => {
            let name = name.to_lowercase();
            !exclude.iter().any(|e| name.contains(e))
        },
        None => {
            true
        },
    }
}

#[inline]
fn collect_ids(tracks: HashMap<&str, Track>) -> Vec<u64> {
    tracks.into_iter().map(|(_, track)| { track.entry.track_number().get() - 1 }).collect()
}

fn modify_or_insert<'a, 'b, F>(
    tracks: &'b mut HashMap<&'a str, Track<'a>>,
    language: &'a str,
    entry: &'a TrackEntry,
    prefer: &'a [String],
    less: F
) where F: Fn(&Track, &Track, &[String]) -> bool {
    let track = Track::new(entry);
    match tracks.get_mut(language) {
        Some(t) => {
            if less(t, &track, prefer) {
                dump("replace", track.entry);
                *t = track
            }
        }
        None => {
            dump("insert", track.entry);
            tracks.insert(language, track);
        }
    }
}

fn tracks<R: Read + Seek>(
    mkv: MatroskaFile<R>,
    languages: &[String],
    exclude: &[String],
    prefer: &[String],
) -> (Vec<u64>, Vec<u64>) {
    let mut audios: HashMap<&str, Track> = HashMap::new();
    let mut subtitles: HashMap<&str, Track> = HashMap::new();
    for entry in mkv.tracks() {
        if let Some(language) = entry.language_bcp47() {
            if check_language(language, languages) && check_exclude(entry.name(), exclude) {
                match entry.track_type() {
                    TrackType::Audio => {
                        modify_or_insert(&mut audios, language, entry, prefer, less_audio);
                    }
                    TrackType::Subtitle => {
                        modify_or_insert(&mut subtitles, language, entry, prefer, less_subtitle);
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
    languages: &[String],
    exclude: &[String],
    prefer: &[String],
    pause: Duration,
    age: Duration
) -> Result<(), MkvPeelError> {
    info!("run, src: {}, dst: {}", src_dir.display(), dst_dir.display());
    let ext_mkv = OsStr::new("mkv");
    loop {
        scan(src_dir, dst_dir, ext_mkv, languages, exclude, prefer, age)?;
        info!("sleep: {} seconds", pause.as_secs());
        sleep(pause);
    }
}

fn scan(
    src_dir: &Path,
    dst_dir: &Path,
    ext_mkv: &OsStr,
    languages: &[String],
    exclude: &[String],
    prefer: &[String],
    age: Duration
) -> Result<(), MkvPeelError> {
    for src_dir_entry in read_dir(src_dir)? {
        let src_dir_entry = src_dir_entry?;
        let src_path = src_dir_entry.path();
        if src_path.is_dir() {
            scan(&src_path, &dst_dir, ext_mkv, languages, exclude, prefer, age)?;
        } else if let Some(ext) = src_path.extension() {
            if ext == ext_mkv {
                let src_meta = metadata(&src_path)?;
                if src_meta.is_file() {
                    if let Some(modified) = src_meta.modified().ok() {
                        if let Some(elapsed) = modified.elapsed().ok() {
                            if elapsed > age {
                                land(&src_path, dst_dir, languages, exclude, prefer)?;
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
    languages: &[String],
    exclude: &[String],
    prefer: &[String],
) -> Result<(), MkvPeelError> {
    info!("land, src: {}, dst: {}", src_path.display(), dst_dir.display());
    let src_file = src_path.file_name().ok_or(MkvPeelError::FileName(src_path.to_path_buf()))?;
    let src_file = src_file.as_bytes();
    let src_file = from_utf8(src_file)?;
    let dst_file = rename(src_file)?;
    let dst_path = dst_dir.join(dst_file);
    if !dst_path.exists() {
        peel(src_path, &dst_path, languages, exclude, prefer)?;
    } else {
        info!("skip, exists: {}", dst_path.display());
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
    let mut year: u64 = 0;
    for c in src.chars() {
        if '0' <= c && c <= '9' && year_unlocked {
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
            if c == '.' {
                year_bracketed = false;
                dst_mkv.push(' ');
            } else {
                year_bracketed = c == '(';
                dst_mkv.push(c);
            }
        }
    }
    dst_mkv.push_str(".mkv");
    info!("rename: '{}' -> '{}'", src_mkv, dst_mkv);
    Ok(dst_mkv)
}

fn peel(
    src_path: &Path,
    dst_path: &Path,
    languages: &[String],
    exclude: &[String],
    prefer: &[String],
) -> Result<(), MkvPeelError> {
    info!("peel, src: '{}', dst: '{}'", src_path.display(), dst_path.display());
    let mut file = File::open(src_path)?;
    match MatroskaFile::open(&mut file) {
        Ok(mkv) => {
        let (audios, subtitles) = tracks(mkv, languages, exclude, prefer);
        info!("peel, audios: {:?}, subtitles: {:?}", audios, subtitles);
        let mut mkvmerge = Command::new("mkvmerge")
            .arg("--output").arg(dst_path)
            .arg("--audio-tracks").arg(join(audios))
            .arg("--subtitle-tracks").arg(join(subtitles))
            .arg(src_path)
            .spawn()?;
        mkvmerge.wait()?;
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
    info!("cmd: {:?}", cmd);
    let src_dir = Path::new(cmd.src.as_str());
    let dst_dir = Path::new(cmd.dst.as_str());
    let languages = to_lowercase(cmd.languages);
    let exclude = to_lowercase(cmd.exclude);
    let mut prefer = to_lowercase(cmd.prefer);
    prefer.reverse();
    let pause = Duration::from(&cmd.pause);
    let age = Duration::from(&cmd.age);
    log(run(src_dir, dst_dir, &languages, &exclude, &prefer, pause, age));
}

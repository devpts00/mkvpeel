use std::cmp::Ordering;
use std::collections::HashMap;
use std::ffi::OsStr;
use std::fmt::{Display, Formatter};
use std::fs::{read_dir, File};
use std::io::{Read, Seek};
use std::fmt::Write;
use std::num::NonZeroU64;
use std::os::unix::ffi::OsStrExt;
use std::path::Path;
use std::process::Command;
use std::str::{from_utf8, FromStr};
use std::thread::sleep;
use std::time::Duration;
use chrono::{Datelike, Utc};
use clap::Parser;
use faststr::FastStr;
use matroska_demuxer::{Audio, MatroskaFile, TrackEntry, TrackType};
use thiserror::Error;
use tracing::info;
use crate::args::{Cmd};
use crate::error::MkvPeelError;
use crate::util::{init_tracing, log};

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
    codec: FastStr,
}

impl UnknownTrackCodecError {
    fn new(track_type: TrackType, codec: &str) -> Self {
        UnknownTrackCodecError { kind: TrackKind::new(track_type), codec: FastStr::new(codec) }
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
fn less_audio(a1: &TrackEntry, a2: &TrackEntry) -> bool {
    let c1: Option<AudioCodec> = a1.codec_id().parse().ok();
    let c2: Option<AudioCodec> = a2.codec_id().parse().ok();
    let d1: Option<AudioDetails> = a1.audio().map(|a| a.into());
    let d2: Option<AudioDetails> = a2.audio().map(|a| a.into());
    (c1, d1) < (c2, d2)
}

fn less_subtitle(s1: &TrackEntry, s2: &TrackEntry) -> bool {
    let c1: Option<SubtitleCodec> = s1.codec_id().parse().ok();
    let c2: Option<SubtitleCodec> = s2.codec_id().parse().ok();
    c1 < c2
}

fn dump(verb: &'static str, track: &TrackEntry) {
    let number = track.track_number().get();
    let language = track.language_bcp47().unwrap_or("n/a");
    let codec = track.codec_id();
    let name = track.name().unwrap_or("n/a");
    let channels = track.audio().map(|a| a.channels());
    info!("{}, track: {}, lang: {}, codec: {}, name: {}, channels: {:?}", verb, number - 1, language, codec, name, channels);
}

fn tracks<R: Read + Seek>(mkv: MatroskaFile<R>, languages: &[FastStr]) -> (Vec<u64>, Vec<u64>) {

    let mut audios = HashMap::new();
    let mut subtitles = HashMap::new();

    for track in mkv.tracks() {
        if let Some(language) = track.language_bcp47() {
            if let Some(language) = languages.iter().find(|l| l.as_str() == language) {
                if languages.contains(language) {
                    match track.track_type() {
                        TrackType::Audio => {
                            audios.entry(language)
                                .and_modify(|t| {
                                    if less_audio(*t, track) {
                                        dump("replace", track);
                                        *t = track
                                    } else {
                                        dump("skip", track);
                                    }
                                })
                                .or_insert_with(|| {
                                    dump("insert", track);
                                    track
                                });
                        }
                        TrackType::Subtitle => {
                            subtitles.entry(language)
                                .and_modify(|t| {
                                    if less_subtitle(*t, track) {
                                        dump("replace", track);
                                        *t = track
                                    } else {
                                        dump("skip", track);
                                    }
                                })
                                .or_insert_with(|| {
                                    dump("insert", track);
                                    track
                                });
                        }
                        _ => {
                        }
                    }
                }
            }
        }
    }

    let audios = audios.into_iter().map(|(_, track)| { track.track_number().get() - 1 }).collect();
    let subtitles = subtitles.into_iter().map(|(_, track)| { track.track_number().get() - 1 }).collect();

    (audios, subtitles)
}

#[inline]
fn join<T: Display>(tracks: Vec<T>) -> String {
    let mut text = String::with_capacity(tracks.len() * 3);
    if !tracks.is_empty() {
        for track in tracks {
            write!(&mut text, "{},", track).unwrap();
        }
        text.truncate(text.len() - 1);
    }
    text
}

fn run(src_dir: &Path, dst_dir: &Path, languages: &[FastStr]) -> Result<(), MkvPeelError> {
    info!("run, src: {}, dst: {}", src_dir.display(), dst_dir.display());
    let ext_mkv = OsStr::new("mkv");
    loop {
        scan(src_dir, dst_dir, ext_mkv, languages)?;
        sleep(Duration::from_secs(10));
    }
}

fn scan(src_dir: &Path, dst_dir: &Path, ext_mkv: &OsStr, languages: &[FastStr]) -> Result<(), MkvPeelError> {
    for src_dir_entry in read_dir(src_dir)? {
        let src_dir_entry = src_dir_entry?;
        let src_path = src_dir_entry.path();
        if src_path.is_dir() {
            scan(&src_path, &dst_dir, ext_mkv, languages)?;
        } else if src_path.is_file() {
            if let Some(ext) = src_path.extension() {
                if ext == ext_mkv {
                    land(&src_path, dst_dir, languages)?;
                }
            }
        }
    }
    Ok(())
}

fn land(src_path: &Path, dst_dir: &Path, languages: &[FastStr]) -> Result<(), MkvPeelError> {
    info!("land, src: {}, dst: {}", src_path.display(), dst_dir.display());
    let src_file = src_path.file_name().ok_or(MkvPeelError::FileName(src_path.to_path_buf()))?;
    let src_file = src_file.as_bytes();
    let src_file = from_utf8(src_file)?;
    let dst_file = rename(src_file)?;
    let dst_path = dst_dir.join(dst_file);
    if !dst_path.exists() {
        peel(src_path, &dst_path, languages)?;
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

fn peel(src_path: &Path, dst_path: &Path, languages: &[FastStr]) -> Result<(), MkvPeelError> {
    info!("peel, src: '{}', dst: '{}'", src_path.display(), dst_path.display());
    let mut file = File::open(src_path)?;
    let mkv = MatroskaFile::open(&mut file)?;
    let (audios, subtitles) = tracks(mkv, languages);
    info!("audios: {:?}", audios);
    info!("subtitles: {:?}", subtitles);
    let mut mkvmerge = Command::new("mkvmerge")
        .arg("--output").arg(dst_path)
        .arg("--audio-tracks").arg(join(audios))
        .arg("--subtitle-tracks").arg(join(subtitles))
        .arg(src_path)
        .spawn()?;
    mkvmerge.wait()?;
    Ok(())
}

fn main() {
    let _guard = init_tracing();
    let cmd = Cmd::parse();
    info!("cmd: {:?}", cmd);
    let src_dir = Path::new(cmd.src.as_str());
    let dst_dir = Path::new(cmd.dst.as_str());
    let languages = &cmd.languages;
    log(run(src_dir, dst_dir, languages));
}

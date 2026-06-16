use std::cmp::Ordering;
use std::collections::HashMap;
use std::fmt::{Display, Formatter};
use std::fs::File;
use std::io::{Read, Seek};
use std::fmt::Write;
use std::num::NonZeroU64;
use std::path::Path;
use std::process::Command;
use std::str::FromStr;
use clap::Parser;
use faststr::FastStr;
use matroska_demuxer::{Audio, MatroskaFile, TrackEntry, TrackType};
use thiserror::Error;
use tracing::{debug, error, info};
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

fn dump(track: &TrackEntry) {
    let number = track.track_number().get();
    let language = track.language_bcp47().unwrap_or("n/a");
    let codec = track.codec_id();
    let name = track.name().unwrap_or("n/a");
    let channels = track.audio().map(|a| a.channels());
    info!("track: {}, lang: {}, codec: {}, name: {}, channels: {:?}", number - 1, language, codec, name, channels);
}

fn tracks<R>(mkv: MatroskaFile<R>, languages: &[FastStr]) -> (Vec<u64>, Vec<u64>)
    where R: Read + Seek {

    let mut audios = HashMap::new();
    let mut subtitles = HashMap::new();

    for track in mkv.tracks() {
        dump(track);
        if let Some(language) = track.language_bcp47() {
            if let Some(language) = languages.iter().find(|l| l.as_str() == language) {
                if languages.contains(language) {
                    match track.track_type() {
                        TrackType::Audio => {
                            //dump(track);
                            audios.entry(language)
                                .and_modify(|t| {
                                    if less_audio(*t, track) {
                                        *t = track
                                    }
                                })
                                .or_insert(track);
                        }
                        TrackType::Subtitle => {
                            //dump(track);
                            subtitles.entry(language)
                                .and_modify(|t| {
                                    if less_subtitle(*t, track) {
                                        *t = track
                                    }
                                })
                                .or_insert(track);
                        }
                        _ => {
                            ;
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
    for track in tracks {
        write!(&mut text, "{},", track).unwrap();
    }
    text.truncate(text.len() - 1);
    text
}

fn run(cmd: Cmd) -> Result<(), MkvPeelError> {
    let mut file = File::open(cmd.src.as_str())?;
    let mkv = MatroskaFile::open(&mut file)?;
    let (audios, subtitles) = tracks(mkv, cmd.languages.as_slice());
    info!("audios: {:?}", audios);
    info!("subtitles: {:?}", subtitles);
    let mut mkvmerge = Command::new("mkvmerge")
        .arg("--output").arg(cmd.dst.as_str())
        .arg("--audio-tracks").arg(join(audios))
        .arg("--subtitle-tracks").arg(join(subtitles))
        .arg(cmd.src.as_str())
        .spawn()?;
    mkvmerge.wait()?;
    Ok(())
}

fn main() {
    let _guard = init_tracing();
    let cmd = Cmd::parse();
    info!("cmd: {:?}", cmd);
    log(run(cmd));
}

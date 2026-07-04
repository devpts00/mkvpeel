use std::io::ErrorKind;
use std::path::Path;
use std::process::Command;
use std::time::Duration;
use bdinfo_rs_core::bdrom::disc::{BdRom, PlaylistSummary, StreamSummary};
use bdinfo_rs_core::stream::TsStreamType;
use bdinfo_rs_core::vfs::fs::FsDir;
use humantime::{format_duration, FormattedDuration};
use isolang::Language;
use regex::Regex;
use tracing::{debug, info, warn};
use crate::error::MkvPeelError;
use crate::peel::{tracks, MkvPeel, Track, TrackBuff, TrackField, TrackKind};
use crate::util::{join, primary_lang};

#[inline]
fn format_secs(seconds: f64) -> FormattedDuration {
    format_duration(Duration::from_secs_f64(seconds))
}

#[inline]
fn check_duration(pls: &PlaylistSummary) -> bool {
    let secs = pls.total_length as u64;
    let min = Duration::from_hours(1).as_secs();
    let max = Duration::from_hours(6).as_secs();
    min <= secs && secs <= max
}

#[inline]
fn video_audio(kind: TsStreamType) -> (bool, bool) {
    match kind {
        TsStreamType::Mpeg1Video => (true, false),
        TsStreamType::Mpeg2Video => (true, false),
        TsStreamType::AvcVideo => (true, false),
        TsStreamType::MvcVideo => (true, false),
        TsStreamType::HevcVideo => (true, false),
        TsStreamType::Vc1Video => (true, false),
        TsStreamType::Mpeg1Audio => (false, true),
        TsStreamType::Mpeg2Audio => (false, true),
        TsStreamType::Mpeg2AacAudio => (false, true),
        TsStreamType::Mpeg4AacAudio => (false, true),
        TsStreamType::LpcmAudio => (false, true),
        TsStreamType::Ac3Audio => (false, true),
        TsStreamType::Ac3PlusAudio => (false, true),
        TsStreamType::Ac3PlusSecondaryAudio => (false, true),
        TsStreamType::Ac3TrueHdAudio => (false, true),
        TsStreamType::DtsAudio => (false, true),
        TsStreamType::DtsHdAudio => (false, true),
        TsStreamType::DtsHdSecondaryAudio => (false, true),
        TsStreamType::DtsHdMasterAudio => (false, true),
        _ => (false, false),
    }
}

#[inline]
fn check_video_audio(pls: &PlaylistSummary) -> bool {
    pls.streams.iter()
        .map(|s| video_audio(s.stream_type) )
        .reduce(|(a1, v1), (a2, v2)| (a1 || a2, v1 || v2))
        .map(|(a, v)| a && v)
        .unwrap_or(false)
}

#[inline]
fn find_best_playlist(playlists: &[PlaylistSummary]) -> Option<&PlaylistSummary> {
    playlists.iter()
        .filter(|pls| !pls.has_loops && check_duration(pls) && check_video_audio(pls))
        .max_by_key(|pls| pls.streams.len())
}

impl Track for StreamSummary {
    fn number(&self) -> Option<u16> {
        None
    }
    fn kind(&self) -> Option<TrackKind> {
        match self.stream_type {
            TsStreamType::Mpeg1Audio => Some(TrackKind::Audio),
            TsStreamType::Mpeg2Audio => Some(TrackKind::Audio),
            TsStreamType::Mpeg2AacAudio => Some(TrackKind::Audio),
            TsStreamType::Mpeg4AacAudio => Some(TrackKind::Audio),
            TsStreamType::LpcmAudio => Some(TrackKind::Audio),
            TsStreamType::Ac3Audio => Some(TrackKind::Audio),
            TsStreamType::Ac3PlusAudio => Some(TrackKind::Audio),
            TsStreamType::Ac3PlusSecondaryAudio => Some(TrackKind::Audio),
            TsStreamType::Ac3TrueHdAudio => Some(TrackKind::Audio),
            TsStreamType::DtsAudio => Some(TrackKind::Audio),
            TsStreamType::DtsHdAudio => Some(TrackKind::Audio),
            TsStreamType::DtsHdSecondaryAudio => Some(TrackKind::Audio),
            TsStreamType::DtsHdMasterAudio => Some(TrackKind::Audio),
            TsStreamType::PresentationGraphics => Some(TrackKind::Subtitles),
            TsStreamType::Subtitle => Some(TrackKind::Subtitles),
            _ => None
        }
    }
    fn lang(&self) -> Option<Language> {
        primary_lang(self.language_code.as_str())
    }
    fn field(&self, field: TrackField) -> Option<&str> {
        match field {
            TrackField::Codec => {
                match self.stream_type {
                    TsStreamType::Mpeg1Audio => Some("A_MPEG/L1"),
                    TsStreamType::Mpeg2Audio => Some("A_MPEG/L2"),
                    TsStreamType::Mpeg2AacAudio => Some("A_AAC/MPEG2"),
                    TsStreamType::Mpeg4AacAudio => Some("A_AAC/MPEG4"),
                    TsStreamType::LpcmAudio => Some("A_PCM"),
                    TsStreamType::Ac3Audio => Some("A_AC3"),
                    TsStreamType::Ac3PlusAudio => Some("A_EAC3"),
                    TsStreamType::Ac3PlusSecondaryAudio => Some("A_EAC3"),
                    TsStreamType::Ac3TrueHdAudio => Some("A_TRUEHD"),
                    TsStreamType::DtsAudio => Some("A_DTS"),
                    TsStreamType::DtsHdAudio => Some("A_DTS"),
                    TsStreamType::DtsHdSecondaryAudio => Some("A_DTS"),
                    TsStreamType::DtsHdMasterAudio => Some("A_DTS"),
                    TsStreamType::PresentationGraphics => Some("S_HDMV/PGS"),
                    TsStreamType::Subtitle => Some("S_TEXT/UTF8"),
                    _ => None
                }
            }
            TrackField::Name => {
                Some(self.description.as_str())
            }
        }
    }
}

pub struct Bdmv;

impl MkvPeel for Bdmv {
    fn probe(&self, path: &Path) -> Result<bool, MkvPeelError> {
        if path.metadata()?.is_dir() {
            match path.join("BDMV").metadata() {
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
            Ok(false)
        }
    }
    fn peel(&self, src: &Path, dst: &Path, langs: &[Language], buffs: &[TrackBuff]) -> Result<(), MkvPeelError> {
        let fsd = FsDir::new(src);
        let disk = BdRom::open(&fsd, false)?;
        match find_best_playlist(&disk.playlists) {
            Some(pls) => {
                info!("playlist, name: {}, duration: {}", pls.name, format_secs(pls.total_length));
                let (audios, subtitles) = tracks(&pls.streams, langs, buffs);
                info!("audios: {:?}, subtitles: {:?}", audios, subtitles);
                let src = src.join("BDMV/PLAYLIST").join(pls.name.to_lowercase());
                let mut mkvmerge = Command::new("mkvmerge");
                mkvmerge.arg("--output").arg(dst);
                if !audios.is_empty() {
                    mkvmerge.arg("--audio-tracks").arg(join(audios));
                }
                if !subtitles.is_empty() {
                    mkvmerge.arg("--subtitle-tracks").arg(join(subtitles));
                }
                mkvmerge.arg(src);
                debug!("run: {:?}", mkvmerge);
                mkvmerge.spawn()?.wait()?;
            }
            None => {
                warn!("failed to find a playlist: {}", src.display());
            }
        }
        Ok(())
    }
}

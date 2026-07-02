use std::path::Path;
use std::time::Duration;
use bdinfo_rs_core::bdrom::disc::{BdRom, PlaylistSummary, StreamSummary};
use bdinfo_rs_core::stream::TsStreamType;
use bdinfo_rs_core::vfs::fs::FsDir;
use humantime::{format_duration, FormattedDuration};
use matroska_demuxer::TrackType;
use regex::Regex;
use tracing::info;
use crate::error::MkvPeelError;
use crate::peel::{MkvPeel, TrackBuff};

pub struct Bdmv;


impl MkvPeel for Bdmv {
    fn probe(&self, path: &Path) -> Result<bool, MkvPeelError> {
        path.join("BDMV").metadata()?;
        Ok(true)
    }
    fn peel(&self, src: &Path, dst: &Path, languages: &[Regex], buffs: &[TrackBuff]) -> Result<(), MkvPeelError> {
        todo!()
    }
}

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

#[derive(Debug)]
struct MkvStreamSummary {
    kind: TrackType,
    codec: &'static str,
    language: String
}

impl TryFrom<StreamSummary> for MkvStreamSummary {
    type Error = ();
    fn try_from(value: StreamSummary) -> Result<Self, Self::Error> {
        todo!()
    }
}

#[inline]
fn ts_to_mkv(kind: TsStreamType) -> Option<(TrackType, &'static str)> {
    match kind {
        TsStreamType::Unknown => None,
        TsStreamType::Mpeg1Video => Some((TrackType::Video, "V_MPEG1")),
        TsStreamType::Mpeg2Video => Some((TrackType::Video, "V_MPEG2")),
        TsStreamType::AvcVideo => Some((TrackType::Video, "V_MPEG4/ISO/AVC")),
        TsStreamType::MvcVideo => Some((TrackType::Video, "V_MPEG4/ISO/AVC")),
        TsStreamType::HevcVideo => Some((TrackType::Video, "V_MPEGH/ISO/HEVC")),
        TsStreamType::Vc1Video => Some((TrackType::Video, "V_MS/VFW/FOURCC")),
        TsStreamType::Mpeg1Audio => Some((TrackType::Audio, "A_MPEG/L1")),
        TsStreamType::Mpeg2Audio => Some((TrackType::Audio, "A_MPEG/L2")),
        TsStreamType::Mpeg2AacAudio => Some((TrackType::Audio, "A_AAC/MPEG2")),
        TsStreamType::Mpeg4AacAudio => Some((TrackType::Audio, "A_AAC/MPEG4")),
        TsStreamType::LpcmAudio => Some((TrackType::Audio, "A_PCM")),
        TsStreamType::Ac3Audio => Some((TrackType::Audio, "A_AC3")),
        TsStreamType::Ac3PlusAudio => Some((TrackType::Audio, "A_EAC3")),
        TsStreamType::Ac3PlusSecondaryAudio => Some((TrackType::Audio, "A_EAC3")),
        TsStreamType::Ac3TrueHdAudio => Some((TrackType::Audio, "A_TRUEHD")),
        TsStreamType::DtsAudio => Some((TrackType::Audio, "A_DTS")),
        TsStreamType::DtsHdAudio => Some((TrackType::Audio, "A_DTS")),
        TsStreamType::DtsHdSecondaryAudio => Some((TrackType::Audio, "A_DTS")),
        TsStreamType::DtsHdMasterAudio => Some((TrackType::Audio, "A_DTS")),
        TsStreamType::PresentationGraphics => Some((TrackType::Subtitle, "S_HDMV/PGS")),
        TsStreamType::InteractiveGraphics => None,
        TsStreamType::Subtitle => Some((TrackType::Subtitle, "S_TEXT/UTF8")),
    }
}

#[inline]
fn check_video_audio(pls: &PlaylistSummary) -> bool {
    pls.streams.iter()
        .filter_map(|s| ts_to_mkv(s.stream_type))
        .map(|(kind, _)| match kind {
            TrackType::Video => (false, true),
            TrackType::Audio => (true, false),
            _ => (false, false)
        })
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

pub fn bdmv(src_dir: &Path) -> Result<(), MkvPeelError> {
    let fsd = FsDir::new(src_dir);
    let disk = BdRom::open(&fsd, false)?;
    if let Some(pls) = find_best_playlist(&disk.playlists) {
        info!("playlist, name: {}, duration: {}", pls.name, format_secs(pls.total_length));
        for stream in &pls.streams {
            info!("    stream, kind: {:?}, language: {}, codec: {}", stream.stream_type, stream.language_code, stream.codec_short_name);
        }
    }
    Ok(())
}

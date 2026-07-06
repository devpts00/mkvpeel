use std::collections::HashMap;
use std::ffi::OsStr;
use std::fs::File;
use std::path::Path;
use std::process::Command;
use isolang::Language;
use matroska_demuxer::{MatroskaFile, TrackEntry, TrackType};
use tracing::{debug, info};
use crate::error::MkvPeelError;
use crate::peel::{collect_track_ids, MkvPeel, Track, TrackBuff, TrackField, TrackKind};
use crate::util::{join, primary_lang};

impl Track for TrackEntry {
    fn number(&self) -> Option<u16> {
        Some(self.track_number().get() as u16 - 1)
    }
    fn kind(&self) -> Option<TrackKind> {
        match self.track_type() {
            TrackType::Audio => Some(TrackKind::Audio),
            TrackType::Subtitle => Some(TrackKind::Subtitles),
            _ => None
        }
    }
    fn lang(&self) -> Option<Language> {
        self.language_bcp47()
            .and_then(primary_lang)
    }
    fn field(&self, field: TrackField) -> Option<&str> {
        match field {
            TrackField::Codec => Some(self.codec_id()),
            TrackField::Name => self.name(),
        }
    }
}

pub struct Mkv;

impl MkvPeel for Mkv {
    fn probe(&self, path: &Path) -> Result<bool, MkvPeelError> {
        let meta = path.metadata()?;
        Ok(meta.is_file() && path.extension().map(|ext| ext.eq_ignore_ascii_case(OsStr::new("mkv"))).unwrap_or(false))
    }
    fn peel(&self, src: &Path, dst: &Path, langs: &[Language], buffs: &[TrackBuff]) -> Result<(), MkvPeelError> {
        info!("peel, src: '{}', dst: '{}'", src.display(), dst.display());
        let mut file = File::open(src)?;
        let mkv = MatroskaFile::open(&mut file)?;

        let tracks = mkv.tracks();
        let size = 2 * tracks.len();

        // TODO: consider adding track order
        let mut audios = HashMap::with_capacity(size);
        let mut subtitles = HashMap::with_capacity(size);
        let mut audio_ids = Vec::with_capacity(size);
        let mut subtitle_ids = Vec::with_capacity(size);
        let mut audio_buf = String::with_capacity(2 * size);
        let mut subtitle_buf = String::with_capacity(2 * size);
        collect_track_ids(tracks, langs, buffs, &mut audios, &mut subtitles, &mut audio_ids, &mut subtitle_ids);

        let mut mkvmerge = Command::new("mkvmerge");
        mkvmerge.arg("--output").arg(dst);
        if !audio_ids.is_empty() {
            join(&audio_ids, &mut audio_buf);
            mkvmerge.arg("--audio-tracks").arg(&audio_buf);
        }
        if !subtitle_ids.is_empty() {
            join(&subtitle_ids, &mut subtitle_buf);
            mkvmerge.arg("--subtitle-tracks").arg(&subtitle_buf);
        }
        mkvmerge.arg(src);
        debug!("run: {:?}", mkvmerge);
        mkvmerge.spawn()?.wait()?;
        Ok(())
    }
}

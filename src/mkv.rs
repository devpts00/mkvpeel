use std::ffi::OsStr;
use std::fs::File;
use std::path::Path;
use std::process::Command;
use isolang::Language;
use matroska_demuxer::{MatroskaFile, TrackEntry, TrackType};
use tracing::{debug, info};
use crate::error::MkvPeelError;
use crate::peel::{tracks, MkvPeel, Track, TrackBuff, TrackField, TrackKind};
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
        // TODO: consider adding track order
        let (audios, subtitles) = tracks(mkv.tracks(), langs, buffs);
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
        Ok(())
    }
}

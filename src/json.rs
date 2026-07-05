use std::ffi::OsStr;
use std::path::Path;
use std::process::Command;
use isolang::Language;
use serde::Deserialize;
use tracing::{debug, info};
use crate::error::MkvPeelError;
use crate::peel::{tracks, MkvPeel, Track, TrackBuff, TrackField, TrackKind};
use crate::util::{join, mkv_probe, pipe, primary_lang};

#[inline]
fn codec_id(codec: &str) -> &str {
    match codec {
        "E-AC-3" => "A_EAC3",
        "Timed Text" => "S_TEXT/UTF8",
        c => c
    }
}

#[derive(Debug, Deserialize)]
#[serde(bound(deserialize = "'de: 'a"))]
struct MkvInfo<'a> {
    tracks: Vec<TrackInfo<'a>>
}

#[derive(Debug, Deserialize)]
struct TrackInfo<'a> {
    id: Option<u16>,
    #[serde(rename(deserialize = "type"))]
    kind: Option<&'a str>,
    codec: Option<&'a str>,
    properties: PropertiesInfo<'a>
}

#[derive(Debug, Deserialize)]
struct PropertiesInfo<'a> {
    codec_id: Option<&'a str>,
    language: Option<&'a str>,
    language_ietf: Option<&'a str>,
    track_name: Option<&'a str>,
}

impl <'a> Track for TrackInfo<'a> {
    fn number(&self) -> Option<u16> {
        self.id
    }
    fn kind(&self) -> Option<TrackKind> {
        match self.kind {
            Some("audio") => Some(TrackKind::Audio),
            Some("subtitles") => Some(TrackKind::Subtitles),
            _ => None
        }
    }
    fn lang(&self) -> Option<Language> {
        self.properties.language_ietf.or(self.properties.language)
            .and_then(|lang| primary_lang(lang) )
    }
    fn field(&self, field: TrackField) -> Option<&str> {
        match field {
            TrackField::Codec => {
                self.properties.codec_id.or(self.codec.map(codec_id))
            }
            TrackField::Name => {
                self.properties.track_name
            }
        }
    }
}

pub struct Json;

impl MkvPeel for Json {
    fn probe(&self, path: &Path) -> Result<bool, MkvPeelError> {
        let extensions = [OsStr::new("mkv"), OsStr::new("mp4"), OsStr::new("avi"), OsStr::new("mov")];
        let meta = path.metadata()?;
        Ok(
            meta.is_file() && path.extension()
                .map(|ext| extensions.iter().any(|e| e.eq_ignore_ascii_case(ext)))
                .unwrap_or(false)
        )
    }
    fn peel(&self, src: &Path, dst: &Path, langs: &[Language], buffs: &[TrackBuff]) -> Result<(), MkvPeelError> {
        let mut buf = String::with_capacity(4 * 1024);
        mkv_probe(src, &mut buf)?;
        let info: MkvInfo = serde_json::from_str(buf.as_str())?;
        let (audios, subtitles) = tracks(&info.tracks, langs, buffs);
        let mut mkvmerge = Command::new("mkvmerge");
        mkvmerge.arg("--verbose");
        mkvmerge.arg("--output").arg(dst);
        if !audios.is_empty() {
            mkvmerge.arg("--audio-tracks").arg(join(audios));
        }
        if !subtitles.is_empty() {
            mkvmerge.arg("--subtitle-tracks").arg(join(subtitles));
        }
        mkvmerge.arg(src);
        debug!("run: {:?}", mkvmerge);
        pipe(mkvmerge)?;
        Ok(())
    }
}

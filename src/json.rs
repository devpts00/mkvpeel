use std::borrow::Cow;
use std::io::{BufReader, Read};
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::Duration;
use isolang::Language;
use serde::Deserialize;
use tracing::warn;
use crate::error::MkvPeelError;
use crate::util::primary_lang;

#[inline]
fn codec_id(codec: &str) -> &str {
    match codec {
        "DTS-HD Master Audio" => {
            "A_DTS"
        },
        "E-AC-3" => {
            "A_EAC3"
        },
        "AC-3" => {
            "A_AC3"
        },
        "FLAC" => {
            "A_FLAC"
        },
        "Timed Text" => {
            "S_TEXT/UTF8"
        },
        "HDMV PGS" => {
            "S_HDMV/PGS"
        },
        c => {
            warn!("unknown codec name: '{}'", c);
            c
        }
    }
}

#[derive(Debug, Deserialize)]
struct CtrPropsInfo {
    playlist_duration: Option<u64>
}

#[derive(Debug, Deserialize)]
struct CtrInfo {
    properties: CtrPropsInfo,
    recognized: bool,
    supported: bool,
}

#[derive(Debug, Deserialize)]
struct TrackPropsInfo<'a> {
    codec_id: Option<&'a str>,
    language: Option<&'a str>,
    language_ietf: Option<&'a str>,
    track_name: Option<Cow<'a, str>>,
    flag_commentary: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct TrackInfo<'a> {
    id: u16,
    #[serde(rename(deserialize = "type"))]
    kind: Option<&'a str>,
    codec: Option<&'a str>,
    properties: TrackPropsInfo<'a>
}

impl <'a> TrackInfo<'a> {
    pub fn id(&self) -> u16 {
        self.id
    }
    pub fn kind(&self) -> Option<&'a str> {
        self.kind
    }
    pub fn codec(&self) -> Option<&'a str> {
        //debug!("codec, id: {:?}, name: {:?}", self.properties.codec_id, self.codec);
        self.properties.codec_id.or_else(|| self.codec.map(codec_id))
    }
    pub fn lang(&self) -> Option<Language> {
        self.properties.language_ietf.and_then(primary_lang)
            .or(self.properties.language.and_then(primary_lang))
    }
    pub fn name(&self) -> Option<&str> {
        self.properties.track_name.as_ref()
            .map(|name| name.as_ref())
    }
    pub fn is_commentary(&self) -> bool {
        self.properties.flag_commentary
            .unwrap_or(false)
    }
}

#[derive(Debug, Deserialize)]
#[serde(bound(deserialize = "'de: 'a"))]
pub struct PlaylistInfo<'a> {
    container: CtrInfo,
    tracks: Vec<TrackInfo<'a>>
}

impl <'a> PlaylistInfo<'a> {
    pub fn tracks(&self) -> &[TrackInfo<'a>] {
        &self.tracks
    }
    pub fn duration(&self) -> Option<Duration> {
        self.container.properties.playlist_duration
            .map(|nanos| Duration::from_nanos(nanos))
    }
    pub fn recognized(&self) -> bool {
        self.container.recognized
    }
    pub fn supported(&self) -> bool {
        self.container.supported
    }
}

impl <'a> PlaylistInfo<'a> {
    pub fn load(path: &Path, buf: &'a mut String) -> Result<PlaylistInfo<'a>, MkvPeelError> {
        buf.clear();
        let mut mkvmerge = Command::new("mkvmerge");
        mkvmerge
            .arg("--output-charset").arg("UTF-8")
            .arg("-J").arg(path);

        let mut mkvmerge = mkvmerge
            .stdout(Stdio::piped())
            .spawn()?;
        if let Some(stdout) = &mut mkvmerge.stdout {
            let mut reader = BufReader::new(stdout);
            reader.read_to_string(buf)?;
        }
        mkvmerge.wait()?;
        //info!("json: {}", buf.as_str());
        let info = serde_json::from_str(buf.as_str())?;
        //info!("info: {:?}", info);
        Ok(info)
    }
}

use std::cmp::min;
use std::error::Error;
use std::ffi::OsStr;
use std::fmt::{Debug, Display, Formatter, Write};
use std::fs::Metadata;
use std::path::Path;
use std::time::Duration;
use chrono::{Datelike, Utc};
use isolang::Language;
use oxilangtag::LanguageTag;
use tracing::level_filters::LevelFilter;
use tracing::{debug, error, warn};
use tracing_subscriber::fmt::format::FmtSpan;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, Layer};
use crate::error::MkvPeelError;

pub fn init_tracing() {
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer()
            .pretty()
            .with_file(false)
            .with_line_number(false)
            .with_thread_ids(true)
            .with_thread_names(true)
            .with_span_events(FmtSpan::NEW | FmtSpan::CLOSE)
            .with_filter(
                EnvFilter::builder()
                    .with_default_directive(LevelFilter::INFO.into())
                    .from_env()
                    .unwrap()
            )
        )
        .init();
}

#[inline]
pub fn log<T: Debug, E: Error>(result: Result<T, E>) {
    match result {
        Ok(value) => {
            debug!("result: {:?}", value);
        },
        Err(err) => {
            error!("error: {}", err)
        }
    }
}

#[inline]
pub fn join<T: Display>(tracks: Vec<T>) -> String {
    let mut text = String::with_capacity(tracks.len() * 3);
    if !tracks.is_empty() {
        for track in tracks {
            write!(&mut text, "{},", track).unwrap();
        }
        text.truncate(text.len() - 1);
    }
    text
}

#[inline]
pub fn write_opt<T: Display>(f: &mut Formatter<'_>, value: Option<T>) -> std::fmt::Result {
    match value {
        Some(value) => write!(f, "{}", value),
        None => write!(f, "und"),
    }
}

pub trait ToOption<U> {
    fn ok_warn<T: Display>(self, ctx: &'static str, t: T) -> Option<U>;
}

impl <U, E: Debug + Display> ToOption<U> for Result<U, E> {
    #[inline]
    fn ok_warn<T: Display>(self, ctx: &'static str, t: T) -> Option<U> {
        match self {
            Ok(u) => {
                Some(u)
            }
            Err(err) => {
                warn!("context: {}, data: {}, error: {:?}", ctx, t, err);
                None
            }
        }
    }
}

#[inline]
pub fn primary_lang(tag: &str) -> Option<Language> {
    LanguageTag::parse(tag)
        .ok_warn("lang", tag)
        .and_then(|tag| {
            let tag = tag.primary_language();
            Language::from_639_1(tag).or(Language::from_639_3(tag))
        })
}

pub fn get_min_age(path: &Path, meta: &Metadata) -> Result<Duration, MkvPeelError> {
    fn _get_min_age_children(path: &Path) -> Result<Duration, MkvPeelError> {
        let mut min_age = Duration::MAX;
        let read = path.read_dir()?;
        for entry in read {
            let entry = entry?;
            let meta = entry.metadata()?;
            let modified = meta.modified()?;
            let age = modified.elapsed()?;
            min_age = min(min_age, age);
            if meta.is_dir() {
                let age = _get_min_age_children(&entry.path())?;
                min_age = min(min_age, age);
            }
        }
        Ok(min_age)
    }
    let modified = meta.modified()?;
    let mut min_age = modified.elapsed()?;
    if meta.is_dir() {
        let age = _get_min_age_children(path)?;
        min_age = min(min_age, age);
    }
    Ok(min_age)
}

pub fn extract_name_without_ext<'a>(path: &'a Path, meta: &Metadata) -> Option<&'a str> {
    path.file_name()
        .and_then(OsStr::to_str)
        .and_then(|name| {
            if meta.is_dir() {
                Some(name)
            } else if let Some(ext) = path.extension() {
                ext.to_str().map(|ext| &name[..name.len() - ext.len()])
            } else {
                Some(name)
            }
        })
}

pub fn make_pretty_name(src: &str) -> Result<String, std::fmt::Error> {
    let mut dst = String::with_capacity(src.len() + 16);
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
                        dst.push('(');
                    }
                    write!(&mut dst, "{}", year)?;
                    dst.push(')');
                    break;
                } else {
                    write!(&mut dst, "{}", year)?;
                    year_in_progress = false;
                    year = 0;
                }
            }
            year_unlocked = true;
            if c == '.' || c.is_whitespace() {
                year_bracketed = false;
                if !whitespace {
                    dst.push(' ');
                    whitespace = true;
                }
            } else {
                year_bracketed = c == '(';
                whitespace = false;
                dst.push(c);
            }
        }
    }
    Ok(dst)
}

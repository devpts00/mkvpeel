use std::cmp::min;
use std::error::Error;
use std::ffi::OsStr;
use std::fmt::{Debug, Display, Write};
use std::fs::Metadata;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Command, Stdio};
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
                ext.to_str().map(|ext| &name[..name.len() - ext.len() - 1])
            } else {
                Some(name)
            }
        })
}

pub fn make_pretty_name(src: &str, dst: &mut String) -> Result<(), std::fmt::Error> {
    dst.clear();
    let year_now = Utc::now().year() as u16;
    let mut year: u16 = 0;
    let mut digits: u8 = 0;
    let mut whitespace = true;

    #[inline]
    fn finish(dst: &mut String, digits: u8, year: u16, year_now: u16) -> Result<bool, std::fmt::Error> {
        if digits == 4 && 1900 <= year && year <= year_now {
            // 4 digit meaningful year means we are done
            // take care of brackets
            if !dst[..dst.len() - 4].ends_with('(') {
                dst.truncate(dst.len() - 4);
                write!(dst, "({}", year)?;
            }
            dst.push(')');
            Ok(true)
        } else {
            Ok(false)
        }
    }

    for c in src.chars() {
        if '0' <= c && c <= '9' {
            // accumulate year up to 4 digits
            if digits < 4 {
                year = 10 * year + (c as u16 - '0' as u16);
            }
            dst.push(c);
            digits += 1;
            whitespace = false;
        } else if finish(dst, digits, year, year_now)? {
            return Ok(())
        } else if c == '_' || c == '.' || c == ' ' {
            if !whitespace {
                dst.push(' ');
                whitespace = true;
            }
            year = 0;
        } else if whitespace {
            for c in c.to_uppercase() {
                dst.push(c);
            }
            whitespace = false;
        } else {
            for c in c.to_lowercase() {
                dst.push(c);
            }
            whitespace = false;
        }
    }

    finish(dst, digits, year, year_now)?;

    Ok(())
}

pub fn pipe(mut cmd: Command) -> Result<(), MkvPeelError> {
    let mut child = cmd.stdout(Stdio::piped())
        .spawn()?;
    if let Some(stdout) = &mut child.stdout {
        let reader = BufReader::new(stdout);
        for _ in reader.lines() {
            // if let Ok(line) = line {
            //     info!("{}: {}", cmd.get_program().display(), line);
            // }
        }
    }
    child.wait()?;
    Ok(())
}

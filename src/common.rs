use std::cmp::min;
use std::ffi::OsStr;
use std::fs::{metadata, rename, Metadata};
use std::path::{Path, PathBuf};
use std::time::{Duration, UNIX_EPOCH};
use std::fmt::Write;
use std::os::unix::prelude::OsStrExt;
use std::str::from_utf8;
use chrono::{Datelike, Utc};
use crate::error::MkvPeelError;
use crate::util::ok_warn;

#[inline]
pub fn try_get_age(path: &Path) -> Result<Duration, MkvPeelError> {
    let meta = metadata(path)?;
    let modified = meta.modified()?;
    let elapsed = modified.elapsed()?;
    Ok(elapsed)
}

#[inline]
pub fn opt_get_age(path: &Path) -> Option<Duration> {
    ok_warn(try_get_age(path))
}

#[inline]
pub fn opt_get_age_ext(path: &Path, ext: &OsStr) -> Option<Duration> {
    path.extension()
        .filter(|e| *e == ext)
        .and_then(|_| opt_get_age(path))
}

#[inline]
pub fn check_ext_age(path: &Path, ext: &OsStr, min_age: &Duration) -> bool {
    opt_get_age_ext(path, ext)
        .map(|age| age >= *min_age)
        .unwrap_or(false)
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

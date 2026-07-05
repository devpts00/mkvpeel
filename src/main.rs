use std::fs::{read_dir};
use std::path::{Path};
use std::thread::sleep;
use std::time::Duration;
use clap::Parser;
use humantime::format_duration;
use isolang::Language;
use tracing::{debug, info, warn};
use crate::args::Cmd;
use crate::bdmv::Bdmv;
use crate::error::MkvPeelError;
use crate::json::Json;
use crate::mkv::Mkv;
use crate::peel::{MkvPeel, TrackBuff};
use crate::util::{init_tracing, log, ToOption, extract_name_without_ext, get_min_age, make_pretty_name};

mod util;
mod args;
mod error;
pub mod bdmv;
pub mod mkv;
pub mod peel;
pub mod json;

#[inline]
fn find<'a>(peels: &'a [Box<dyn MkvPeel>], src_path: &Path) -> Option<&'a Box<dyn MkvPeel>> {
    peels.iter().find(|peel|
        peel.probe(src_path).ok_warn("probe", src_path.display()).unwrap_or(false)
    )
}

fn scan(
    peels: &[Box<dyn MkvPeel>],
    src_dir: &Path,
    dst_dir: &Path,
    languages: &[Language],
    buffs: &[TrackBuff],
    min_age: Duration
) -> Result<(), MkvPeelError> {
    for src_dir_entry in read_dir(src_dir)? {
        let src_dir_entry = src_dir_entry?;
        let src_meta = src_dir_entry.metadata()?;
        let src_path = src_dir_entry.path();
        debug!("found: {}", src_path.display());
        match find(peels, &src_path) {
            Some(peel) => {
                if let Some(age) = get_min_age(&src_path, &src_meta).ok_warn("age", src_path.display()) {
                    if age >= min_age {
                        match extract_name_without_ext(&src_path, &src_meta) {
                            Some(src_name) => {
                                if let Some(mut dst_name) = make_pretty_name(src_name).ok_warn("prettify", src_name) {
                                    dst_name.push_str(".mkv");
                                    let dst_path = dst_dir.join(&dst_name);
                                    if !dst_path.exists() {
                                        peel.peel(&src_path, &dst_path, languages, buffs).ok_warn("peel", src_path.display());
                                    } else {
                                        debug!("exists: {}", dst_path.display());
                                    }
                                }
                            }
                            None => {
                                warn!("name: {}", src_path.display());
                            }
                        }
                    } else {
                        debug!("waiting, age: {}, path: {}", format_duration(age), src_path.display());
                    }
                }
            }
            None => {
                if src_meta.is_dir() {
                    scan(peels, &src_path, dst_dir, languages, buffs, min_age)?;
                }
            }
        }
    }
    Ok(())
}

fn run(
    peels: &[Box<dyn MkvPeel>],
    src_dir: &Path,
    dst_dir: &Path,
    languages: &[Language],
    buff: &[TrackBuff],
    pause: Duration,
    age: Duration
) -> Result<(), MkvPeelError> {
    info!("run, src: {}, dst: {}", src_dir.display(), dst_dir.display());
    loop {
        scan(peels, src_dir, dst_dir, languages, buff, age)?;
        debug!("sleep: {} seconds", pause.as_secs());
        sleep(pause);
    }
}

fn main() {
    let _guard = init_tracing();
    let cmd = Cmd::parse();
    debug!("cmd: {:?}", cmd);
    let peels: Vec<Box<dyn MkvPeel>> = vec!(Box::new(Bdmv), Box::new(Json), Box::new(Mkv));
    let src_dir = Path::new(cmd.src.as_str());
    let dst_dir = Path::new(cmd.dst.as_str());
    let languages = cmd.languages;
    let buff = cmd.buff;
    let pause = Duration::from(&cmd.pause);
    let age = Duration::from(&cmd.age);
    log(run(&peels, src_dir, dst_dir, &languages, &buff, pause, age));
}

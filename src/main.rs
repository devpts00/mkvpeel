use std::fs::{read_dir};
use std::path::{Path};
use std::thread::sleep;
use std::time::Duration;
use clap::Parser;
use humantime::format_duration;
use tracing::{debug, info, warn};
use crate::args::{Cmd};
use crate::error::MkvPeelError;
use crate::json::JsonImpl;
use crate::util::{init_tracing, ToOption, extract_name_without_ext, get_min_age, make_pretty_name, log};

mod util;
mod args;
mod error;
pub mod json;
pub mod model;

fn scan(
    peel: &mut JsonImpl,
    src_dir: &Path,
    dst_dir: &Path,
    min_age: Duration
) -> Result<(), MkvPeelError> {
    for src_dir_entry in read_dir(src_dir)? {
        let src_dir_entry = src_dir_entry?;
        let src_meta = src_dir_entry.metadata()?;
        let src_path = src_dir_entry.path();
        debug!("found: {}", src_path.display());
        match peel.check(&src_path) {
            Ok(yes) => {
                if yes {
                    if let Some(age) = get_min_age(&src_path, &src_meta).ok_warn("age", src_path.display()) {
                        if age >= min_age {
                            match extract_name_without_ext(&src_path, &src_meta) {
                                Some(src_name) => {
                                    if let Some(mut dst_name) = make_pretty_name(src_name).ok_warn("prettify", src_name) {
                                        dst_name.push_str(".mkv");
                                        let dst_path = dst_dir.join(&dst_name);
                                        if !dst_path.exists() {
                                            peel.peel(&src_path, &dst_path).ok_warn("peel", src_path.display());
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
                } else if src_meta.is_dir() {
                    scan(peel, &src_path, dst_dir, min_age)?;
                }
            }
            Err(err) => {
                warn!("check: {}", err);
            }
        }
    }
    Ok(())
}

fn run(
    peels: &mut JsonImpl,
    src_dir: &Path,
    dst_dir: &Path,
    pause: Duration,
    age: Duration
) -> Result<(), MkvPeelError> {
    info!("run, src: {}, dst: {}", src_dir.display(), dst_dir.display());
    loop {
        scan(peels, src_dir, dst_dir, age)?;
        debug!("sleep: {} seconds", pause.as_secs());
        sleep(pause);
    }
}

fn main() {
    let _guard = init_tracing();
    let cmd = Cmd::parse();
    debug!("cmd: {:?}", cmd);
    let src_dir = Path::new(cmd.src.as_str());
    let dst_dir = Path::new(cmd.dst.as_str());
    let langs = cmd.languages;
    let codecs = cmd.codec;
    let names = cmd.name;
    let mut json_impl = JsonImpl::new(langs, codecs, names);
    let pause = Duration::from(&cmd.pause);
    let age = Duration::from(&cmd.age);
    log(run(&mut json_impl, src_dir, dst_dir, pause, age));
}

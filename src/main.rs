use clap::Parser;
use tracing::debug;
use crate::args::{Cmd, Proto};
use crate::util::{init_tracing, log};

mod util;
mod args;
mod error;

fn main() {
    let _guard = init_tracing();
    let cmd = Cmd::parse();
    debug!("cmd: {:?}", cmd);
}

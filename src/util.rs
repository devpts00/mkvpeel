use std::error::Error;
use std::fmt::{Debug, Display, Write};
use tracing::level_filters::LevelFilter;
use tracing::{debug, error};
use tracing_subscriber::fmt::format::FmtSpan;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, Layer};

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
pub fn to_lowercase(ss: Vec<String>) -> Vec<String> {
    ss.into_iter().map(|s| s.to_lowercase()).collect()
}
use clap::{Parser, Subcommand};
use clap_duration::duration_range_value_parse;
use duration_human::{DurationHuman, DurationHumanValidator};
use faststr::FastStr;

#[derive(Parser, Debug)]
#[command(version, about)]
pub struct Cmd {
    #[arg(long)]
    pub src: FastStr,
    #[arg(long)]
    pub dst: FastStr,
    #[arg(long, value_delimiter = ',')]
    pub languages: Vec<FastStr>,
    #[arg(long, default_value = "60s", value_parser = duration_range_value_parse!(min: 10s, max: 10min))]
    pub pause: DurationHuman,
    #[arg(long, default_value = "60s", value_parser = duration_range_value_parse!(min: 10s, max: 60min))]
    pub age: DurationHuman,
}

#[derive(Subcommand, Debug)]
pub enum Proto {
}
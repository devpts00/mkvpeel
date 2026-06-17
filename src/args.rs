use clap::Parser;
use clap_duration::duration_range_value_parse;
use duration_human::{DurationHuman, DurationHumanValidator};

#[derive(Parser, Debug)]
#[command(version, about)]
pub struct Cmd {
    #[arg(long)]
    pub src: String,
    #[arg(long)]
    pub dst: String,
    #[arg(long, value_delimiter = ',')]
    pub languages: Vec<String>,
    #[arg(long, value_delimiter = ',', default_value = "commentary")]
    pub exclude: Vec<String>,
    #[arg(long, value_delimiter = ',', default_value = "пучков,full")]
    pub prefer: Vec<String>,
    #[arg(long, default_value = "60s", value_parser = duration_range_value_parse!(min: 10s, max: 10min))]
    pub pause: DurationHuman,
    #[arg(long, default_value = "60s", value_parser = duration_range_value_parse!(min: 10s, max: 60min))]
    pub age: DurationHuman,
}

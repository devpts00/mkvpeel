use clap::{Parser, Subcommand};
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
}

#[derive(Subcommand, Debug)]
pub enum Proto {
}
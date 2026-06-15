use std::net::{Ipv4Addr, Ipv6Addr};
use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(version, about)]
pub struct Cmd {
}

#[derive(Subcommand, Debug)]
pub enum Proto {
}
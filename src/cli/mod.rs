mod auth;
mod config;
mod list;
mod pull;
mod push;
mod root;

use anyhow::Result;
use clap::ArgMatches;

use crate::app::App;

pub fn build_cli() -> clap::Command {
    root::build_root_command()
}

pub fn execute(app: &App, matches: ArgMatches) -> Result<()> {
    match matches.subcommand() {
        Some(("auth", sub_matches)) => auth::handle(app, sub_matches),
        Some(("config", sub_matches)) => config::handle(app, sub_matches),
        Some(("ls", sub_matches)) => list::handle(app, sub_matches),
        Some(("pull", sub_matches)) => pull::handle(app, sub_matches),
        Some(("push", sub_matches)) => push::handle(app, sub_matches),
        _ => Ok(()),
    }
}

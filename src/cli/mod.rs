mod auth;
mod config;
mod info;
mod list;
mod mkdir;
mod mkrepo;
mod pull;
mod push;
mod repos;
mod root;
mod share;
mod shares;
mod unshare;

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
        Some(("download", sub_matches)) => pull::handle(app, sub_matches),
        Some(("info", sub_matches)) => info::handle(app, sub_matches),
        Some(("ls", sub_matches)) => list::handle(app, sub_matches),
        Some(("mkdir", sub_matches)) => mkdir::handle(app, sub_matches),
        Some(("mkrepo", sub_matches)) => mkrepo::handle(app, sub_matches),
        Some(("upload", sub_matches)) => push::handle(app, sub_matches),
        Some(("repos", sub_matches)) => repos::handle(app, sub_matches),
        Some(("share", sub_matches)) => share::handle(app, sub_matches),
        Some(("shares", sub_matches)) => shares::handle(app, sub_matches),
        Some(("unshare", sub_matches)) => unshare::handle(app, sub_matches),
        _ => Ok(()),
    }
}

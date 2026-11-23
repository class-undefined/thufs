use std::path::PathBuf;

use anyhow::{Result, bail};
use clap::{Arg, ArgAction, ArgMatches, Command};

pub fn build_command() -> Command {
    Command::new("tui")
        .about("Open the interactive cloud drive workspace")
        .long_about("Browse libraries, queue file transfers, and manage share links in an interactive terminal. Sign in with a token inside the TUI or use saved credentials. Press Ctrl+P for commands, 4 for language and account settings, or ? for help.")
        .arg(Arg::new("remote").help("Start at a library or repo:<library>/<directory>"))
        .arg(Arg::new("local").long("local").value_name("DIRECTORY").value_parser(clap::value_parser!(PathBuf)).help("Local directory for uploads and downloads (default: current directory)"))
        .arg(Arg::new("demo").long("demo").action(ArgAction::SetTrue).help("Explore a read-only offline demo without credentials"))
        .arg(Arg::new("no-mouse").long("no-mouse").action(ArgAction::SetTrue).help("Disable mouse capture to allow normal terminal text selection"))
}

pub fn handle(matches: &ArgMatches) -> Result<()> {
    if matches.get_flag("json") {
        bail!(
            "tui does not support --json; use repos, ls, upload, download, or shares for scriptable output"
        );
    }
    crate::tui::run(crate::tui::Options {
        remote: matches.get_one::<String>("remote").cloned(),
        local: matches.get_one::<PathBuf>("local").cloned(),
        demo: matches.get_flag("demo"),
        mouse: !matches.get_flag("no-mouse"),
    })
}

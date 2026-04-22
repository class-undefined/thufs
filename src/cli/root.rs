use clap::{Arg, ArgAction, Command};

use super::{auth, config, info, list, mkdir, mkrepo, pull, push, repos, share, shares, unshare};

pub fn build_root_command() -> Command {
    Command::new("thufs")
        .about("THU Cloud Drive CLI for shell-first file workflows")
        .long_about(
            "thufs keeps daily THU Cloud Drive work compact in the terminal.\n\n\
Business verbs stay flat and scriptable: info, repos, ls, upload, download, share, shares, unshare, mkrepo, and mkdir.\n\
Management verbs stay grouped under auth and config.",
        )
        .arg(
            Arg::new("json")
                .long("json")
                .help("Render command results as JSON")
                .global(true)
                .action(ArgAction::SetTrue),
        )
        .subcommand(info::build_command())
        .subcommand(repos::build_command())
        .subcommand(list::build_command())
        .subcommand(mkrepo::build_command())
        .subcommand(mkdir::build_command())
        .subcommand(push::build_command())
        .subcommand(pull::build_command())
        .subcommand(share::build_command())
        .subcommand(shares::build_command())
        .subcommand(unshare::build_command())
        .subcommand(auth::build_command())
        .subcommand(config::build_command())
        .subcommand_required(false)
        .arg_required_else_help(true)
}

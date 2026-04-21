use clap::{Arg, ArgAction, Command};

use super::{auth, config};

pub fn build_root_command() -> Command {
    Command::new("thufs")
        .about("THU Cloud Drive CLI for shell-first file workflows")
        .long_about(
            "thufs keeps daily THU Cloud Drive work compact in the terminal.\n\n\
Business verbs stay flat and scriptable: push, pull, ls, and share.\n\
Management verbs stay grouped under auth and config.",
        )
        .arg(
            Arg::new("json")
                .long("json")
                .help("Render command results as JSON")
                .global(true)
                .action(ArgAction::SetTrue),
        )
        .subcommand(auth::build_command())
        .subcommand(config::build_command())
        .subcommand_required(false)
        .arg_required_else_help(true)
}

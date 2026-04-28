use std::path::PathBuf;

use anyhow::Result;
use clap::{Arg, ArgAction, ArgMatches, Command, value_parser};

use crate::app::App;

pub fn build_command() -> Command {
    Command::new("upload")
        .about("Upload a local file to THU Cloud Drive")
        .visible_alias("push")
        .arg(Arg::new("local").help("Local source file").required(true))
        .arg(
            Arg::new("remote")
                .help("Remote file path, remote directory, or repo root in repo:<library>/<path> form or default-repo shorthand")
                .required(true),
        )
        .arg(
            Arg::new("conflict")
                .long("conflict")
                .value_name("POLICY")
                .help("Conflict policy: uniquify, overwrite, fail, or prompt")
                .value_parser(value_parser!(String))
                .conflicts_with_all(["overwrite", "rename", "fail"]),
        )
        .arg(
            Arg::new("progress")
                .long("progress")
                .value_name("MODE")
                .help("Progress output: auto, jsonl, or none. jsonl streams machine-readable progress events to stderr")
                .value_parser(value_parser!(String)),
        )
        .arg(
            Arg::new("overwrite")
                .long("overwrite")
                .hide(true)
                .help("Replace the remote file if it already exists")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("rename")
                .long("rename")
                .hide(true)
                .help("Deprecated alias for --conflict=uniquify")
                .conflicts_with("overwrite")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("fail")
                .long("fail")
                .hide(true)
                .help("Fail immediately if the remote file already exists")
                .conflicts_with_all(["overwrite", "rename"])
                .action(ArgAction::SetTrue),
        )
}

pub fn handle(app: &App, matches: &ArgMatches) -> Result<()> {
    let local = PathBuf::from(
        matches
            .get_one::<String>("local")
            .expect("required by clap"),
    );
    let remote = matches
        .get_one::<String>("remote")
        .expect("required by clap");
    let conflict_policy = crate::transfer::conflict_policy_from_matches(matches)?;
    let progress_mode = crate::transfer::progress_mode_from_matches(matches)?;

    let result = app
        .push_service
        .push(&local, remote, conflict_policy, progress_mode)?;
    let mut stdout = std::io::stdout();
    if matches.get_flag("json") {
        app.renderer.write_json(&mut stdout, &result)?;
    } else {
        app.renderer.write_line(
            &mut stdout,
            &format!(
                "Uploaded {} to {}{} ({})",
                result.local_path,
                result.repo,
                result.remote_path,
                if result.uniquified {
                    "uniquified"
                } else if result.overwritten {
                    "overwritten"
                } else {
                    "created"
                }
            ),
        )?;
    }
    Ok(())
}

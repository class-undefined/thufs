use std::path::PathBuf;

use anyhow::Result;
use clap::{Arg, ArgAction, ArgMatches, Command};

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
            Arg::new("overwrite")
                .long("overwrite")
                .help("Replace the remote file if it already exists")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("rename")
                .long("rename")
                .help("Pick a unique remote name instead of overwriting")
                .conflicts_with("overwrite")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("fail")
                .long("fail")
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
    let conflict_policy = if matches.get_flag("overwrite") {
        crate::transfer::ConflictPolicy::Overwrite
    } else if matches.get_flag("rename") {
        crate::transfer::ConflictPolicy::Rename
    } else if matches.get_flag("fail") {
        crate::transfer::ConflictPolicy::Fail
    } else {
        crate::transfer::ConflictPolicy::Prompt
    };

    let result = app.push_service.push(&local, remote, conflict_policy)?;
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
                if result.renamed {
                    "renamed"
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

use std::path::PathBuf;

use anyhow::Result;
use clap::{Arg, ArgAction, ArgMatches, Command};

use crate::app::App;

pub fn build_command() -> Command {
    Command::new("download")
        .about("Download a remote file from THU Cloud Drive")
        .visible_alias("pull")
        .arg(
            Arg::new("remote")
                .help("Remote file path in repo:<library>/<path> form or default-repo shorthand")
                .required(true),
        )
        .arg(
            Arg::new("local")
                .help("Local destination file or existing directory")
                .required(false),
        )
        .arg(
            Arg::new("overwrite")
                .long("overwrite")
                .help("Replace the local file if it already exists")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("rename")
                .long("rename")
                .help("Pick a unique local name instead of overwriting")
                .conflicts_with("overwrite")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("fail")
                .long("fail")
                .help("Fail immediately if the local file already exists")
                .conflicts_with_all(["overwrite", "rename"])
                .action(ArgAction::SetTrue),
        )
}

pub fn handle(app: &App, matches: &ArgMatches) -> Result<()> {
    let remote = matches
        .get_one::<String>("remote")
        .expect("required by clap");
    let local = matches.get_one::<String>("local").map(PathBuf::from);
    let conflict_policy = if matches.get_flag("overwrite") {
        crate::transfer::ConflictPolicy::Overwrite
    } else if matches.get_flag("rename") {
        crate::transfer::ConflictPolicy::Rename
    } else if matches.get_flag("fail") {
        crate::transfer::ConflictPolicy::Fail
    } else {
        crate::transfer::ConflictPolicy::Prompt
    };

    let result = app
        .pull_service
        .pull(remote, local.as_deref(), conflict_policy)?;
    let mut stdout = std::io::stdout();
    if matches.get_flag("json") {
        app.renderer.write_json(&mut stdout, &result)?;
    } else {
        app.renderer.write_line(
            &mut stdout,
            &format!(
                "Downloaded {}{} to {}{}",
                result.repo,
                result.remote_path,
                result.local_path,
                if result.renamed { " (renamed)" } else { "" }
            ),
        )?;
    }
    Ok(())
}

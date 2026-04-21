use std::path::PathBuf;

use anyhow::Result;
use clap::{Arg, ArgAction, ArgMatches, Command};

use crate::app::App;

pub fn build_command() -> Command {
    Command::new("push")
        .about("Upload a local file to THU Cloud Drive")
        .arg(Arg::new("local").help("Local source file").required(true))
        .arg(
            Arg::new("remote")
                .help("Remote file path in repo:<library>/<path> form or default-repo shorthand")
                .required(true),
        )
        .arg(
            Arg::new("overwrite")
                .long("overwrite")
                .help("Replace the remote file if it already exists")
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
    let overwrite = matches.get_flag("overwrite");

    let result = app.push_service.push(&local, remote, overwrite)?;
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
                if result.overwritten {
                    "overwritten"
                } else {
                    "created"
                }
            ),
        )?;
    }
    Ok(())
}

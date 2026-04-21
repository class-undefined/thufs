use std::path::PathBuf;

use anyhow::Result;
use clap::{Arg, ArgAction, ArgMatches, Command};

use crate::app::App;

pub fn build_command() -> Command {
    Command::new("pull")
        .about("Download a remote file from THU Cloud Drive")
        .arg(
            Arg::new("remote")
                .help("Remote file path in repo:<library>/<path> form or default-repo shorthand")
                .required(true),
        )
        .arg(
            Arg::new("local")
                .help("Local destination file or existing directory")
                .required(true),
        )
        .arg(
            Arg::new("overwrite")
                .long("overwrite")
                .help("Replace the local file if it already exists")
                .action(ArgAction::SetTrue),
        )
}

pub fn handle(app: &App, matches: &ArgMatches) -> Result<()> {
    let remote = matches
        .get_one::<String>("remote")
        .expect("required by clap");
    let local = PathBuf::from(
        matches
            .get_one::<String>("local")
            .expect("required by clap"),
    );
    let overwrite = matches.get_flag("overwrite");

    let result = app.pull_service.pull(remote, &local, overwrite)?;
    let mut stdout = std::io::stdout();
    if matches.get_flag("json") {
        app.renderer.write_json(&mut stdout, &result)?;
    } else {
        app.renderer.write_line(
            &mut stdout,
            &format!(
                "Downloaded {}{} to {}",
                result.repo, result.remote_path, result.local_path
            ),
        )?;
    }
    Ok(())
}

use anyhow::{Result, bail};
use clap::{Arg, ArgMatches, Command};

use crate::app::App;

pub fn build_command() -> Command {
    Command::new("mkdir")
        .about("Create a remote directory, creating parents as needed")
        .arg(
            Arg::new("remote")
                .help(
                    "Remote directory path in repo:<library>/<path> form or default-repo shorthand",
                )
                .required(true),
        )
}

pub fn handle(app: &App, matches: &ArgMatches) -> Result<()> {
    let remote = matches
        .get_one::<String>("remote")
        .expect("required by clap");
    if remote.trim() == "/" {
        bail!("cannot create remote root directory");
    }

    let result = app.create_service.create_dir(remote)?;
    let mut stdout = std::io::stdout();
    if matches.get_flag("json") {
        app.renderer.write_json(&mut stdout, &result)?;
    } else {
        app.renderer.write_line(
            &mut stdout,
            &format!("Created {}{}", result.repo, result.path),
        )?;
    }
    Ok(())
}

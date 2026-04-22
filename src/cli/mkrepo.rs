use anyhow::Result;
use clap::{Arg, ArgMatches, Command};

use crate::app::App;

pub fn build_command() -> Command {
    Command::new("mkrepo")
        .about("Create a library or repository")
        .visible_alias("mklib")
        .arg(
            Arg::new("name")
                .help("Library name to create")
                .required(true),
        )
}

pub fn handle(app: &App, matches: &ArgMatches) -> Result<()> {
    let name = matches.get_one::<String>("name").expect("required by clap");
    let result = app.create_service.create_repo(name)?;
    let mut stdout = std::io::stdout();
    if matches.get_flag("json") {
        app.renderer.write_json(&mut stdout, &result)?;
    } else {
        let line = if result.created {
            format!("Created repo {}\t{}", result.repo, result.repo_id)
        } else {
            format!("Repo already exists {}\t{}", result.repo, result.repo_id)
        };
        app.renderer.write_line(&mut stdout, &line)?;
    }
    Ok(())
}

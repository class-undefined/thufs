use anyhow::Result;
use clap::{Arg, ArgMatches, Command};

use crate::{app::App, config::ConfigManager};

pub fn build_command() -> Command {
    Command::new("ls")
        .about("List remote files and directories")
        .arg(
            Arg::new("remote")
                .help("Remote path in repo:<library>/<path> form, repo root like <library>, or default-repo shorthand")
                .required(true),
        )
}

pub fn handle(app: &App, matches: &ArgMatches) -> Result<()> {
    let remote = matches
        .get_one::<String>("remote")
        .expect("required by clap");

    let config = ConfigManager::new();
    let result = app.list_service.list(remote, &config)?;

    let mut stdout = std::io::stdout();
    if matches.get_flag("json") {
        app.renderer.write_json(&mut stdout, &result)?;
    } else {
        app.renderer.write_line(
            &mut stdout,
            &crate::app::list_service::ListService::format_human(&result),
        )?;
    }
    Ok(())
}

use anyhow::Result;
use clap::{Arg, ArgAction, ArgMatches, Command};

use crate::{app::App, config::ConfigManager};

pub fn build_command() -> Command {
    Command::new("ls")
        .about("List remote files and directories")
        .arg(
            Arg::new("remote")
                .help("Remote path in repo:<library>/<path> form, repo root like <library>, or default-repo shorthand")
                .required(true),
        )
        .arg(
            Arg::new("time")
                .long("time")
                .short('t')
                .help("Show update time for each entry")
                .action(ArgAction::SetTrue),
        )
}

pub fn handle(app: &App, matches: &ArgMatches) -> Result<()> {
    let remote = matches
        .get_one::<String>("remote")
        .expect("required by clap");

    let config = ConfigManager::new();
    let result = app
        .list_service
        .list(remote, &config, matches.get_flag("time"))?;

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

use anyhow::Result;
use clap::{ArgMatches, Command};

use crate::{app::App, app::account_service::AccountService};

pub fn build_command() -> Command {
    Command::new("repos")
        .about("List repositories or libraries visible to the current token")
        .visible_alias("libraries")
}

pub fn handle(app: &App, matches: &ArgMatches) -> Result<()> {
    let result = app.account_service.repositories()?;
    let mut stdout = std::io::stdout();
    if matches.get_flag("json") {
        app.renderer.write_json(&mut stdout, &result)?;
    } else {
        app.renderer
            .write_line(&mut stdout, &AccountService::format_repositories(&result))?;
    }
    Ok(())
}

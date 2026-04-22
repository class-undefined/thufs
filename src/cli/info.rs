use anyhow::Result;
use clap::{ArgMatches, Command};

use crate::{app::App, app::account_service::AccountService};

pub fn build_command() -> Command {
    Command::new("info").about("Show account information for the current token")
}

pub fn handle(app: &App, matches: &ArgMatches) -> Result<()> {
    let result = app.account_service.account_info()?;
    let mut stdout = std::io::stdout();
    if matches.get_flag("json") {
        app.renderer.write_json(&mut stdout, &result)?;
    } else {
        app.renderer
            .write_line(&mut stdout, &AccountService::format_account_info(&result))?;
    }
    Ok(())
}

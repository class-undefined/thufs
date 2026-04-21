use anyhow::Result;
use clap::{ArgMatches, Command};

use crate::app::App;

pub fn build_command() -> Command {
    Command::new("config")
        .about("Grouped management commands for local thufs settings")
        .long_about(
            "Inspect and manage file-first thufs configuration.\n\n\
Environment variables remain an override layer, but local config stays the primary path.",
        )
        .subcommand(Command::new("show").about("Show the active resolved configuration"))
        .arg_required_else_help(true)
}

pub fn handle(app: &App, matches: &ArgMatches) -> Result<()> {
    match matches.subcommand() {
        Some(("show", _)) => {
            let inspection = app.auth_service.inspect()?;
            let json = matches.get_flag("json");

            let mut stdout = std::io::stdout();
            if json {
                app.renderer.write_json(&mut stdout, &inspection)?;
            } else {
                app.renderer.write_line(
                    &mut stdout,
                    &format!(
                        "Config file: {}\nToken: {}\nDefault repo: {}\nOutput: {}\nEnvironment overrides: {}",
                        inspection.config_path,
                        inspection.token,
                        inspection
                            .default_repo
                            .clone()
                            .unwrap_or_else(|| "not set".to_string()),
                        inspection.output,
                        if inspection.environment_overrides.is_empty() {
                            "none".to_string()
                        } else {
                            inspection.environment_overrides.join(", ")
                        }
                    ),
                )?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

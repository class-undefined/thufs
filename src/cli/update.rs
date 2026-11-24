use anyhow::Result;
use clap::{Arg, ArgAction, ArgMatches, Command};

use crate::app::{App, update_service::CheckMode};

pub fn build_command() -> Command {
    Command::new("update")
        .about("Check for new thufs releases")
        .subcommand(
            Command::new("check")
                .about("Check the latest stable GitHub release without installing it")
                .arg(
                    Arg::new("force")
                        .long("force")
                        .action(ArgAction::SetTrue)
                        .help("Check online now instead of using the 24-hour cache"),
                ),
        )
        .arg_required_else_help(true)
}

pub fn handle(app: &App, matches: &ArgMatches) -> Result<()> {
    if let Some(("check", arguments)) = matches.subcommand() {
        let mode = if arguments.get_flag("force") {
            CheckMode::Refresh
        } else {
            CheckMode::Cached
        };
        let result = app.update_service.check(mode)?;
        let mut stdout = std::io::stdout();
        if arguments.get_flag("json") {
            app.renderer.write_json(&mut stdout, &result)?;
        } else {
            let status = if result.update_available {
                format!(
                    "Update available: {} → {}",
                    result.current_version,
                    result.latest_version.as_deref().unwrap_or_default()
                )
            } else if result.latest_version.is_none() {
                "No stable release has been published yet.".into()
            } else {
                format!("thufs {} is up to date.", result.current_version)
            };
            app.renderer.write_line(&mut stdout, &status)?;
            app.renderer
                .write_line(&mut stdout, &format!("Releases: {}", result.release_url))?;
            if result.update_available {
                app.renderer
                    .write_line(&mut stdout, "Cargo: cargo install thufs --locked --force")?;
            }
            if result.cached {
                app.renderer.write_line(
                    &mut stdout,
                    "Using a cached check; use --force to check online now.",
                )?;
            }
        }
    }
    Ok(())
}

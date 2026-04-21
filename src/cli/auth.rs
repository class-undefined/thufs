use anyhow::Result;
use clap::{Arg, ArgMatches, Command};

use crate::app::App;

pub fn build_command() -> Command {
    Command::new("auth")
        .about("Grouped management commands for token setup")
        .long_about(
            "Manage token-driven authentication settings for thufs.\n\n\
v1 uses a pre-issued token instead of password or browser login flows.",
        )
        .subcommand(
            Command::new("set-token")
                .about("Store the THU Cloud Drive access token")
                .arg(
                    Arg::new("token")
                        .help("Access token obtained outside thufs")
                        .required(true),
                ),
        )
        .arg_required_else_help(true)
}

pub fn handle(app: &App, matches: &ArgMatches) -> Result<()> {
    match matches.subcommand() {
        Some(("set-token", sub_matches)) => {
            let token = sub_matches
                .get_one::<String>("token")
                .expect("required by clap");
            let result = app.auth_service.set_token(token)?;
            let json = matches.get_flag("json");

            let mut stdout = std::io::stdout();
            if json {
                app.renderer.write_json(&mut stdout, &result)?;
            } else {
                app.renderer.write_line(
                    &mut stdout,
                    &format!("Stored token {} in {}", result.token, result.config_path),
                )?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

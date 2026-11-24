use anyhow::{Context, Result};
use clap::{Arg, ArgMatches, Command};

use crate::app::App;

pub fn build_command() -> Command {
    Command::new("config")
        .about("Grouped management commands for local thufs settings")
        .long_about(
            "Inspect and manage file-first thufs configuration.\n\n\
Environment variables remain an override layer, but local config stays the primary path.",
        )
        .subcommand(Command::new("show").about("Show the active resolved configuration"))
        .subcommand(
            Command::new("proxy")
                .about("Set the proxy used by the CLI and TUI")
                .subcommand(
                    Command::new("auto")
                        .visible_alias("detect")
                        .about("Automatically use proxy environment variables or system settings"),
                )
                .subcommand(Command::new("direct").about("Connect directly without any proxy"))
                .subcommand(
                    Command::new("manual")
                        .about("Use an HTTP, HTTPS, or SOCKS5 proxy")
                        .arg(
                            Arg::new("url")
                                .required(true)
                                .help("Proxy URL, for example http://127.0.0.1:7890"),
                        ),
                )
                .arg_required_else_help(true),
        )
        .arg_required_else_help(true)
}

pub fn handle(app: &App, matches: &ArgMatches) -> Result<()> {
    match matches.subcommand() {
        Some(("proxy", arguments)) => {
            let (mode, sub) = arguments.subcommand().context("Choose a proxy mode")?;
            let address = if mode == "manual" {
                sub.get_one::<String>("url")
                    .map(String::as_str)
                    .unwrap_or_default()
            } else {
                ""
            };
            let proxy = crate::network::ProxyConfig::from_input(mode, address)?;
            app.auth_service.set_proxy(&proxy)?;
            let mut stdout = std::io::stdout();
            if matches.get_flag("json") {
                app.renderer.write_json(
                    &mut stdout,
                    &serde_json::json!({"proxy": proxy.description()}),
                )?;
            } else {
                app.renderer
                    .write_line(&mut stdout, &format!("Proxy: {}", proxy.description()))?;
            }
            Ok(())
        }
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
                        "Config file: {}\nToken: {}\nDefault repo: {}\nOutput: {}\nUser-Agent: {}\nProxy: {}\nEnvironment overrides: {}",
                        inspection.config_path,
                        inspection.token,
                        inspection
                            .default_repo
                            .clone()
                            .unwrap_or_else(|| "not set".to_string()),
                        inspection.output,
                        inspection.user_agent,
                        inspection.proxy,
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

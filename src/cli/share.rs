use anyhow::{Context, Result};
use clap::{Arg, ArgMatches, Command, value_parser};

use crate::app::{App, share_service::ShareOptions};

pub fn build_command() -> Command {
    Command::new("share")
        .about("Create a THU Cloud Drive share link")
        .arg(
            Arg::new("remote")
                .help("Remote file or directory in repo:<library>/<path> form or default-repo shorthand")
                .required(true),
        )
        .arg(
            Arg::new("password")
                .long("password")
                .help("Protect the share link with a password")
                .value_name("PASSWORD"),
        )
        .arg(
            Arg::new("expire-days")
                .long("expire-days")
                .help("Expire the share link after this many days")
                .value_name("DAYS")
                .value_parser(value_parser!(u32)),
        )
}

pub fn handle(app: &App, matches: &ArgMatches) -> Result<()> {
    let remote = matches
        .get_one::<String>("remote")
        .expect("required by clap");
    let password = matches.get_one::<String>("password").cloned();
    let expire_days = matches.get_one::<u32>("expire-days").copied();

    let result = app.share_service.share(
        remote,
        ShareOptions {
            password,
            expire_days,
        },
    )?;

    let mut stdout = std::io::stdout();
    if matches.get_flag("json") {
        app.renderer.write_json(&mut stdout, &result)?;
    } else {
        app.renderer
            .write_line(&mut stdout, &result.link)
            .context("failed to write share link")?;
    }
    Ok(())
}

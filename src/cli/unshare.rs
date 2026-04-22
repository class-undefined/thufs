use anyhow::Result;
use clap::{Arg, ArgMatches, Command};

use crate::app::App;

pub fn build_command() -> Command {
    Command::new("unshare")
        .about("Delete a THU Cloud Drive share link")
        .arg(
            Arg::new("token")
                .help("Share link token to delete")
                .required(true),
        )
}

pub fn handle(app: &App, matches: &ArgMatches) -> Result<()> {
    let token = matches
        .get_one::<String>("token")
        .expect("required by clap");
    let result = app.share_service.unshare(token)?;

    let mut stdout = std::io::stdout();
    if matches.get_flag("json") {
        app.renderer.write_json(&mut stdout, &result)?;
    } else {
        app.renderer
            .write_line(&mut stdout, &format!("Deleted share link {}", result.token))?;
    }
    Ok(())
}

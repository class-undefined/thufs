use anyhow::Result;
use clap::{Arg, ArgAction, ArgMatches, Command, value_parser};

use crate::app::{App, share_service::ShareListResult};

pub fn build_command() -> Command {
    Command::new("shares")
        .about("List THU Cloud Drive share links")
        .arg(
            Arg::new("remote")
                .help("Optional library, file, or directory path to inspect")
                .required(false),
        )
        .arg(
            Arg::new("page")
                .long("page")
                .help("Page number when listing share links")
                .value_name("N")
                .value_parser(value_parser!(usize)),
        )
        .arg(
            Arg::new("per-page")
                .long("per-page")
                .help("Number of share links per page")
                .value_name("N")
                .value_parser(value_parser!(usize)),
        )
        .arg(
            Arg::new("all")
                .long("all")
                .help("List all share links")
                .action(ArgAction::SetTrue),
        )
}

pub fn handle(app: &App, matches: &ArgMatches) -> Result<()> {
    let remote = matches.get_one::<String>("remote").map(String::as_str);
    let page = matches.get_one::<usize>("page").copied().unwrap_or(1);
    let per_page = matches.get_one::<usize>("per-page").copied().unwrap_or(50);
    let result = app
        .share_service
        .list_shares(remote, page, per_page, matches.get_flag("all"))?;

    let mut stdout = std::io::stdout();
    if matches.get_flag("json") {
        app.renderer.write_json(&mut stdout, &result)?;
    } else {
        app.renderer
            .write_line(&mut stdout, &format_human(&result))?;
    }
    Ok(())
}

fn format_human(result: &ShareListResult) -> String {
    if result.shares.is_empty() {
        return "No share links found".to_string();
    }

    let mut lines = vec![format!(
        "share links page={} per_page={} total={} has_more={}",
        result.page, result.per_page, result.total, result.has_more
    )];
    lines.extend(
        result
            .shares
            .iter()
            .map(|share| {
                format!(
                    "{}\t{}\t{}\t{}",
                    share.token.as_deref().unwrap_or("-"),
                    share
                        .repo
                        .as_deref()
                        .unwrap_or(share.repo_id.as_deref().unwrap_or("-")),
                    share.path.as_deref().unwrap_or("-"),
                    share.url
                )
            })
            .collect::<Vec<_>>(),
    );
    lines.join("\n")
}

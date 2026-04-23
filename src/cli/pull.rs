use std::path::PathBuf;

use anyhow::Result;
use clap::{Arg, ArgAction, ArgMatches, Command, value_parser};

use crate::app::App;

pub fn build_command() -> Command {
    Command::new("download")
        .about("Download a remote file from THU Cloud Drive")
        .visible_alias("pull")
        .arg(
            Arg::new("remote")
                .help("Remote file path, share URL, or share hashcode")
                .required(true),
        )
        .arg(
            Arg::new("local")
                .help("Local destination file or existing directory")
                .required(false),
        )
        .arg(
            Arg::new("mode")
                .long("mode")
                .value_name("MODE")
                .help("Download mode: auto, parallel, or sequential")
                .value_parser(value_parser!(String)),
        )
        .arg(
            Arg::new("workers")
                .long("workers")
                .value_name("N")
                .help("Number of parallel download workers")
                .value_parser(value_parser!(usize)),
        )
        .arg(
            Arg::new("share")
                .long("share")
                .help("Interpret the remote argument as a share hashcode or share URL")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("conflict")
                .long("conflict")
                .value_name("POLICY")
                .help("Conflict policy: uniquify, overwrite, fail, or prompt")
                .value_parser(value_parser!(String))
                .conflicts_with_all(["overwrite", "rename", "fail"]),
        )
        .arg(
            Arg::new("overwrite")
                .long("overwrite")
                .hide(true)
                .help("Replace the local file if it already exists")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("rename")
                .long("rename")
                .hide(true)
                .help("Deprecated alias for --conflict=uniquify")
                .conflicts_with("overwrite")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("fail")
                .long("fail")
                .hide(true)
                .help("Fail immediately if the local file already exists")
                .conflicts_with_all(["overwrite", "rename"])
                .action(ArgAction::SetTrue),
        )
}

pub fn handle(app: &App, matches: &ArgMatches) -> Result<()> {
    let remote = matches
        .get_one::<String>("remote")
        .expect("required by clap");
    let local = matches.get_one::<String>("local").map(PathBuf::from);
    let conflict_policy = crate::transfer::conflict_policy_from_matches(matches)?;
    let download_mode = matches
        .get_one::<String>("mode")
        .map(|value| crate::transfer::DownloadMode::parse_keyword(value))
        .transpose()?
        .unwrap_or(crate::transfer::DownloadMode::Auto);
    let workers = matches.get_one::<usize>("workers").copied().unwrap_or(4);
    let from_share = matches.get_flag("share");

    let result = app.pull_service.pull(
        remote,
        local.as_deref(),
        from_share,
        conflict_policy,
        download_mode,
        workers,
    )?;
    let mut stdout = std::io::stdout();
    if matches.get_flag("json") {
        app.renderer.write_json(&mut stdout, &result)?;
    } else {
        app.renderer.write_line(
            &mut stdout,
            &format!(
                "Downloaded {} to {}{}",
                result.source,
                result.local_path,
                if result.uniquified {
                    " (uniquified)"
                } else {
                    ""
                }
            ),
        )?;
    }
    Ok(())
}

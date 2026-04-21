mod app;
mod cli;
mod config;
mod contract;
mod output;

use anyhow::Result;
use app::App;

fn main() {
    if let Err(err) = run() {
        eprintln!("{err}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let app = App::new();
    let cli = cli::build_cli();
    let matches = cli.get_matches();
    cli::execute(&app, matches)
}

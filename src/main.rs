//! upm — Universal Package Manager.

mod app;
mod backend;
mod capabilities;
mod cli;
mod config;
mod detect;
mod error;
mod exec;
mod history;
mod manifest;
mod models;
mod output;
mod parse;
mod paths;
mod plan;
mod registry;
mod resolver;
mod validate;

use std::process::ExitCode;

use clap::Parser;

use crate::cli::Cli;
use crate::config::Config;
use crate::error::code_of;

fn main() -> ExitCode {
    let cli = Cli::parse();

    let loaded = Config::load();
    let color = loaded.as_ref().ok().and_then(|cfg| cfg.general.color);
    output::init(cli.no_color, color, cli.json, cli.quiet, cli.verbose);

    let cfg = match loaded {
        Ok(cfg) => cfg,
        Err(err) => return report(&err),
    };

    match app::run(cli, cfg) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => report(&err),
    }
}

fn report(err: &anyhow::Error) -> ExitCode {
    let code = code_of(err);
    let hint = error::hint_of(err);
    if output::is_json() {
        let payload = serde_json::json!({
            "schema_version": 1,
            "status": "error",
            "exit_code": code,
            "error": format!("{err:#}"),
            "hint": hint,
        });
        println!("{payload}");
    } else {
        output::error(&format!("{err:#}"));
        if let Some(hint) = hint {
            eprintln!("  {} {hint}", output::dim("hint:"));
        }
    }
    ExitCode::from(code as u8)
}

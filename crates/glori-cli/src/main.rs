mod cli;
mod commands;
mod output;

use std::process::ExitCode;

use clap::Parser;

use cli::Cli;

fn main() -> ExitCode {
    let cli = Cli::parse();
    if cli.no_color {
        owo_colors::set_override(false);
    }
    commands::run(cli).unwrap_or_else(|error| {
        output::error(&error);
        ExitCode::FAILURE
    })
}

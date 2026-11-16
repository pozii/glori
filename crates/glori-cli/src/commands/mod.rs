mod completions;
mod doctor;
mod info;
mod install;

use std::process::ExitCode;

use crate::cli::{Cli, Command};

pub fn run(cli: Cli) -> anyhow::Result<ExitCode> {
    match cli.command {
        Command::Info { file } => info::run(&file, cli.json).map(|()| ExitCode::SUCCESS),
        Command::Install {
            file,
            dry_run,
            yes,
            allow_untrusted,
        } => install::run(
            &file,
            install::Options {
                dry_run,
                yes,
                allow_untrusted,
                json: cli.json,
            },
        )
        .map(|()| ExitCode::SUCCESS),
        Command::Doctor => doctor::run(cli.json),
        Command::Completions { shell } => {
            completions::run(shell);
            Ok(ExitCode::SUCCESS)
        }
    }
}

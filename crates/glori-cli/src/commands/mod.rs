mod completions;
mod doctor;
mod execute;
mod info;
mod install;
mod list;
mod remove;

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
            name,
        } => install::run(
            &file,
            install::Options {
                dry_run,
                yes,
                allow_untrusted,
                json: cli.json,
                name,
            },
        )
        .map(|()| ExitCode::SUCCESS),
        Command::Remove { name, dry_run, yes } => remove::run(
            &name,
            remove::Options {
                dry_run,
                yes,
                json: cli.json,
            },
        )
        .map(|()| ExitCode::SUCCESS),
        Command::List => list::run(cli.json).map(|()| ExitCode::SUCCESS),
        Command::Doctor => doctor::run(cli.json),
        Command::Completions { shell } => {
            completions::run(shell);
            Ok(ExitCode::SUCCESS)
        }
    }
}

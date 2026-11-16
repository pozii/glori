mod completions;
mod doctor;
mod execute;
mod info;
mod install;
mod libraries;
mod list;
mod remove;
mod self_update;
mod source;

use std::process::ExitCode;

use crate::cli::{Cli, Command};

pub fn run(cli: Cli) -> anyhow::Result<ExitCode> {
    match cli.command {
        Command::Info { source, sha256 } => {
            info::run(&source, sha256, cli.json).map(|()| ExitCode::SUCCESS)
        }
        Command::Install {
            source,
            dry_run,
            yes,
            allow_untrusted,
            sha256,
            install_deps,
            name,
        } => install::run(
            &source,
            install::Options {
                dry_run,
                yes,
                allow_untrusted,
                json: cli.json,
                name,
                sha256,
                install_deps,
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
        Command::SelfUpdate { check, yes } => self_update::run(self_update::Options {
            check,
            yes,
            json: cli.json,
        })
        .map(|()| ExitCode::SUCCESS),
        Command::List => list::run(cli.json).map(|()| ExitCode::SUCCESS),
        Command::Doctor => doctor::run(cli.json),
        Command::Completions { shell } => {
            completions::run(shell);
            Ok(ExitCode::SUCCESS)
        }
    }
}

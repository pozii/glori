use std::path::PathBuf;

use clap::{Parser, Subcommand};
use clap_complete::Shell;

#[derive(Parser)]
#[command(
    name = "glori",
    version,
    about = "Install software the same way on every Linux distribution",
    propagate_version = true,
    arg_required_else_help = true
)]
pub struct Cli {
    #[arg(long, global = true, help = "Print machine-readable JSON")]
    pub json: bool,

    #[arg(long, global = true, help = "Disable colored output")]
    pub no_color: bool,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    #[command(
        about = "Show the detected format, distribution and install plan for a file",
        after_help = "Examples:\n  glori info ./app.deb\n  glori info ./app.tar.gz --json"
    )]
    Info {
        #[arg(help = "Path to the package or archive")]
        file: PathBuf,
    },

    #[command(
        about = "Install a package from a local file",
        after_help = "Examples:\n  glori install ./app.rpm\n  glori install ./app.deb --dry-run\n  glori install ./app.rpm --yes"
    )]
    Install {
        #[arg(help = "Path to the package")]
        file: PathBuf,

        #[arg(long, help = "Show the plan without changing anything")]
        dry_run: bool,

        #[arg(short, long, help = "Do not ask for confirmation")]
        yes: bool,
    },

    #[command(about = "Check the system and the tools Glori relies on")]
    Doctor,

    #[command(
        about = "Generate shell completions",
        after_help = "Examples:\n  glori completions bash > ~/.local/share/bash-completion/completions/glori\n  glori completions zsh > ~/.zfunc/_glori"
    )]
    Completions {
        #[arg(value_enum, help = "Target shell")]
        shell: Shell,
    },
}

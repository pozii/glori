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
        after_help = "Examples:\n  glori info ./app.deb\n  glori info ./app.tar.gz --sha256 <hash>\n  glori info ./app.tar.gz --json"
    )]
    Info {
        #[arg(help = "Path to the package or archive")]
        file: PathBuf,

        #[arg(long, value_name = "HEX", value_parser = parse_sha256, help = "Expected SHA-256 of the file")]
        sha256: Option<String>,
    },

    #[command(
        about = "Install a package, archive or AppImage from a local file",
        after_help = "Examples:\n  glori install ./app.rpm\n  glori install ./tool-1.2.tar.gz --dry-run\n  glori install ./App.AppImage --name myapp --yes\n  glori install ./tool.tar.gz --sha256 <hash>\n  glori install ./app.tar.xz --install-deps"
    )]
    Install {
        #[arg(help = "Path to the package")]
        file: PathBuf,

        #[arg(long, help = "Show the plan without changing anything")]
        dry_run: bool,

        #[arg(short, long, help = "Do not ask for confirmation")]
        yes: bool,

        #[arg(long, value_name = "HEX", value_parser = parse_sha256, help = "Expected SHA-256 of the file; the install is refused if it differs")]
        sha256: Option<String>,

        #[arg(
            long,
            help = "Install even if the signature is missing or the verification fails"
        )]
        allow_untrusted: bool,

        #[arg(
            long,
            help = "Install the missing system libraries first, using the package manager"
        )]
        install_deps: bool,

        #[arg(
            long,
            help = "Name to install under instead of the one derived from the file"
        )]
        name: Option<String>,
    },

    #[command(
        about = "Remove something that Glori installed",
        after_help = "Examples:\n  glori remove ripgrep\n  glori remove ripgrep --dry-run"
    )]
    Remove {
        #[arg(help = "Name shown by `glori list`")]
        name: String,

        #[arg(long, help = "Show the plan without changing anything")]
        dry_run: bool,

        #[arg(short, long, help = "Do not ask for confirmation")]
        yes: bool,
    },

    #[command(about = "List what Glori installed")]
    List,

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

fn parse_sha256(text: &str) -> Result<String, String> {
    if glori_core::verify::checksum::is_sha256(text) {
        Ok(text.to_lowercase())
    } else {
        Err("expected 64 hexadecimal characters".to_string())
    }
}

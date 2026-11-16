use std::io;

use clap::CommandFactory;
use clap_complete::{Shell, generate};

use crate::cli::Cli;

pub fn run(shell: Shell) {
    generate(shell, &mut Cli::command(), "glori", &mut io::stdout());
}

use std::io::{self, IsTerminal, Write};

use anyhow::bail;

pub fn confirm(question: &str) -> anyhow::Result<bool> {
    if !io::stdin().is_terminal() {
        bail!("confirmation required but input is not a terminal; rerun with --yes");
    }
    print!("{question} [y/N] ");
    io::stdout().flush()?;
    let mut answer = String::new();
    io::stdin().read_line(&mut answer)?;
    Ok(matches!(answer.trim().to_lowercase().as_str(), "y" | "yes"))
}

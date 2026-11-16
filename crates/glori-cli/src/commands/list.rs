use glori_core::{Paths, registry::Registry};

use crate::{output, timestamp};

pub fn run(json: bool) -> anyhow::Result<()> {
    let paths = Paths::from_env()?;
    let registry = Registry::load(&paths.registry_file())?;
    if json {
        return output::json(&registry.entries());
    }
    if registry.entries().is_empty() {
        println!("Nothing installed by Glori yet.");
        return Ok(());
    }
    println!(
        "{}",
        output::bold(&format!(
            "{:<20} {:<12} {:<9} {:<11} SOURCE",
            "NAME", "VERSION", "TYPE", "INSTALLED"
        ))
    );
    for entry in registry.entries() {
        println!(
            "{:<20} {:<12} {:<9} {:<11} {}",
            entry.name,
            entry.version.as_deref().unwrap_or("-"),
            entry.kind.label(),
            timestamp::date(entry.installed_at),
            output::dim(&entry.source)
        );
    }
    Ok(())
}

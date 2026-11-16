use anyhow::bail;
use glori_core::{Elevation, Error, Operation, Paths, distro, privilege, remove};
use serde::Serialize;

use super::execute;
use crate::{output, prompt};

#[derive(Serialize)]
struct Plan<'a> {
    name: &'a str,
    operations: &'a [Operation],
}

pub struct Options {
    pub dry_run: bool,
    pub yes: bool,
    pub json: bool,
}

pub fn run(name: &str, options: Options) -> anyhow::Result<()> {
    let Options { dry_run, yes, json } = options;
    if json && !dry_run {
        bail!("--json is only supported together with --dry-run");
    }
    let paths = Paths::from_env()?;
    let operations = remove::plan(name, &paths).map_err(|error| hint(error, name))?;
    if json {
        return output::json(&Plan {
            name,
            operations: &operations,
        });
    }
    println!("{} {name}", output::bold("Removing"));
    execute::print_operations(&operations, &Ok(Elevation::Direct));
    if dry_run {
        println!("{}", output::dim("Dry run: nothing was changed."));
        return Ok(());
    }
    if !yes && !prompt::confirm("Proceed?")? {
        bail!("cancelled, nothing was changed");
    }
    let elevation = execute::needs_elevation(&operations, privilege::detect())?;
    execute::execute(&operations, elevation, paths)
}

fn hint(error: Error, name: &str) -> anyhow::Error {
    if !matches!(error, Error::NotInstalled(_)) {
        return error.into();
    }
    let manager = distro::detect()
        .ok()
        .and_then(|distro| distro.family.package_manager())
        .map_or("your package manager", |manager| manager.binary());
    anyhow::anyhow!("{error} (if {name} came from a system package, remove it with {manager})")
}

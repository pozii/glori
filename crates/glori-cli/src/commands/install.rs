use std::path::Path;

use anyhow::bail;
use glori_core::{Elevation, Step, installer, privilege, runner};
use serde::Serialize;

use crate::{output, prompt, report::Summary};

#[derive(Serialize)]
struct Plan<'a> {
    #[serde(flatten)]
    summary: &'a Summary<'a>,
    elevation: Option<Elevation>,
    steps: &'a [Step],
}

pub fn run(file: &Path, dry_run: bool, yes: bool, json: bool) -> anyhow::Result<()> {
    if json && !dry_run {
        bail!("--json is only supported together with --dry-run");
    }
    let summary = Summary::detect(file)?;
    let steps = installer::select(summary.strategy)?.steps(file)?;
    let elevation = privilege::detect();
    if json {
        return output::json(&Plan {
            summary: &summary,
            elevation: elevation.as_ref().ok().copied(),
            steps: &steps,
        });
    }
    summary.print();
    print_steps(&steps, &elevation);
    if dry_run {
        println!("{}", output::dim("Dry run: nothing was changed."));
        return Ok(());
    }
    let elevation = needs_elevation(&steps, elevation)?;
    if !yes && !prompt::confirm("Proceed?")? {
        bail!("cancelled, nothing was changed");
    }
    execute(&steps, elevation)
}

fn needs_elevation(
    steps: &[Step],
    detected: glori_core::Result<Elevation>,
) -> anyhow::Result<Elevation> {
    if steps.iter().any(|step| step.privileged) {
        Ok(detected?)
    } else {
        Ok(detected.unwrap_or(Elevation::Direct))
    }
}

fn print_steps(steps: &[Step], elevation: &glori_core::Result<Elevation>) {
    println!("\n{}", output::bold("Steps"));
    let shown = elevation.as_ref().ok().copied();
    for (index, step) in steps.iter().enumerate() {
        println!("  {}. {}", index + 1, step.command_line(shown));
    }
    if elevation.is_err() && steps.iter().any(|step| step.privileged) {
        println!(
            "{}",
            output::yellow("  root is required but neither sudo nor pkexec was found")
        );
    }
    println!();
}

fn execute(steps: &[Step], elevation: Elevation) -> anyhow::Result<()> {
    for (index, step) in steps.iter().enumerate() {
        println!(
            "{} {}",
            output::bold(&format!("[{}/{}]", index + 1, steps.len())),
            step.command_line(Some(elevation))
        );
        runner::run(step, elevation)?;
    }
    println!("{}", output::green("Done."));
    Ok(())
}

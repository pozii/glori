use std::path::Path;

use anyhow::bail;
use glori_core::{Elevation, Step, Trust, installer, privilege, runner};
use serde::Serialize;

use crate::{output, prompt, report::Summary};

#[derive(Serialize)]
struct Plan<'a> {
    #[serde(flatten)]
    summary: &'a Summary<'a>,
    elevation: Option<Elevation>,
    steps: &'a [Step],
}

pub struct Options {
    pub dry_run: bool,
    pub yes: bool,
    pub allow_untrusted: bool,
    pub json: bool,
}

pub fn run(file: &Path, options: Options) -> anyhow::Result<()> {
    let Options {
        dry_run,
        yes,
        allow_untrusted,
        json,
    } = options;
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
    let question = trust_question(summary.signature, allow_untrusted, yes)?;
    let elevation = needs_elevation(&steps, elevation)?;
    if !yes && !prompt::confirm(question)? {
        bail!("cancelled, nothing was changed");
    }
    execute(&steps, elevation)
}

fn trust_question(trust: Trust, allow_untrusted: bool, yes: bool) -> anyhow::Result<&'static str> {
    match (trust, allow_untrusted, yes) {
        (Trust::Invalid, false, _) => bail!(
            "the signature check failed; refusing to install (use --allow-untrusted to override)"
        ),
        (Trust::Unsigned, false, true) => {
            bail!("the package is not signed; rerun with --allow-untrusted to install it anyway")
        }
        (Trust::Unsigned, false, false) => Ok("This package is not signed. Install it anyway?"),
        (Trust::Invalid | Trust::Unsigned, true, _) => Ok("The package is untrusted. Proceed?"),
        _ => Ok("Proceed?"),
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verified_and_unverifiable_packages_use_the_normal_prompt() {
        for trust in [Trust::Verified, Trust::Unverifiable] {
            assert_eq!(trust_question(trust, false, false).unwrap(), "Proceed?");
            assert_eq!(trust_question(trust, false, true).unwrap(), "Proceed?");
        }
    }

    #[test]
    fn invalid_signatures_are_refused_without_the_override() {
        assert!(trust_question(Trust::Invalid, false, false).is_err());
        assert!(trust_question(Trust::Invalid, false, true).is_err());
        assert!(trust_question(Trust::Invalid, true, true).is_ok());
    }

    #[test]
    fn unsigned_packages_need_the_override_in_scripts() {
        assert!(trust_question(Trust::Unsigned, false, true).is_err());
        assert!(trust_question(Trust::Unsigned, true, true).is_ok());
    }

    #[test]
    fn unsigned_packages_get_a_dedicated_question_when_interactive() {
        assert_eq!(
            trust_question(Trust::Unsigned, false, false).unwrap(),
            "This package is not signed. Install it anyway?"
        );
    }
}

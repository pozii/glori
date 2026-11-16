use std::path::Path;

use anyhow::bail;
use glori_core::{
    Elevation, Operation, Paths, Trust,
    installer::{self, Input},
    privilege,
};
use serde::Serialize;

use super::execute;
use crate::{output, prompt, report::Summary};

#[derive(Serialize)]
struct Plan<'a> {
    #[serde(flatten)]
    summary: &'a Summary<'a>,
    elevation: Option<Elevation>,
    operations: &'a [Operation],
}

pub struct Options {
    pub dry_run: bool,
    pub yes: bool,
    pub allow_untrusted: bool,
    pub json: bool,
    pub name: Option<String>,
}

pub fn run(file: &Path, options: Options) -> anyhow::Result<()> {
    let Options {
        dry_run,
        yes,
        allow_untrusted,
        json,
        name,
    } = options;
    if json && !dry_run {
        bail!("--json is only supported together with --dry-run");
    }
    let summary = Summary::detect(file)?;
    let paths = Paths::from_env()?;
    let input = Input {
        file,
        format: summary.format,
        name: name.as_deref(),
        paths: &paths,
    };
    let operations = installer::select(summary.strategy)?.plan(&input)?;
    let elevation = privilege::detect();
    if json {
        return output::json(&Plan {
            summary: &summary,
            elevation: elevation.as_ref().ok().copied(),
            operations: &operations,
        });
    }
    summary.print();
    execute::print_operations(&operations, &elevation);
    if dry_run {
        println!("{}", output::dim("Dry run: nothing was changed."));
        return Ok(());
    }
    let question = trust_question(summary.signature, allow_untrusted, yes)?;
    let elevation = execute::needs_elevation(&operations, elevation)?;
    if !yes && !prompt::confirm(question)? {
        bail!("cancelled, nothing was changed");
    }
    execute::execute(&operations, elevation, paths)
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

use std::path::Path;

use anyhow::bail;
use glori_core::{
    Elevation, Operation, Paths, Trust,
    installer::{self, Input},
    libs, privilege, verify,
};
use serde::Serialize;

use super::{execute, libraries};
use crate::{output, prompt, report::Summary};

#[derive(Serialize)]
struct Plan<'a> {
    #[serde(flatten)]
    summary: &'a Summary<'a>,
    elevation: Option<Elevation>,
    operations: &'a [Operation],
    libraries: &'a libs::Report,
}

pub struct Options {
    pub dry_run: bool,
    pub yes: bool,
    pub allow_untrusted: bool,
    pub json: bool,
    pub name: Option<String>,
    pub sha256: Option<String>,
    pub install_deps: bool,
}

pub fn run(file: &Path, options: Options) -> anyhow::Result<()> {
    let Options {
        dry_run,
        yes,
        allow_untrusted,
        json,
        name,
        sha256,
        install_deps,
    } = options;
    if json && !dry_run {
        bail!("--json is only supported together with --dry-run");
    }
    let summary = Summary::detect(file, &verify::Options { sha256 })?;
    let paths = Paths::from_env()?;
    let manager = summary.distro.family.package_manager();
    let input = Input {
        file,
        format: summary.format,
        name: name.as_deref(),
        paths: &paths,
        manager,
    };
    let plan = installer::select(summary.strategy)?.plan(&input)?;
    let (mut operations, libraries) = (plan.operations, plan.libraries);
    let packages = libraries.packages();
    let deps_step = manager
        .filter(|_| install_deps && !packages.is_empty())
        .map(|manager| Operation::Run(installer::install_packages(manager, &packages)));
    let deps_will_be_installed = deps_step.is_some();
    operations.splice(0..0, deps_step);
    let elevation = privilege::detect();
    if json {
        return output::json(&Plan {
            summary: &summary,
            elevation: elevation.as_ref().ok().copied(),
            operations: &operations,
            libraries: &libraries,
        });
    }
    summary.print();
    execute::print_operations(&operations, &elevation);
    libraries::print(&libraries, manager, elevation.as_ref().ok().copied());
    if install_deps && !deps_will_be_installed && !libraries.missing.is_empty() {
        println!(
            "{}",
            output::yellow(
                "--install-deps has nothing to install: no providing packages were found"
            )
        );
    }
    if dry_run {
        println!("{}", output::dim("Dry run: nothing was changed."));
        return Ok(());
    }
    let question = libraries::question(
        &libraries,
        deps_will_be_installed,
        trust_question(summary.verification, allow_untrusted, yes)?,
    );
    let elevation = execute::needs_elevation(&operations, elevation)?;
    if !yes && !prompt::confirm(question)? {
        bail!("cancelled, nothing was changed");
    }
    execute::execute(&operations, elevation, paths)
}

fn trust_question(trust: Trust, allow_untrusted: bool, yes: bool) -> anyhow::Result<&'static str> {
    match (trust, allow_untrusted, yes) {
        (Trust::Invalid, false, _) => {
            bail!("verification failed; refusing to install (use --allow-untrusted to override)")
        }
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

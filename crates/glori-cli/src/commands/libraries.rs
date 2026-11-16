use glori_core::{Elevation, PackageManager, installer, libs::Report};

use crate::output;

pub fn print(report: &Report, manager: Option<PackageManager>, elevation: Option<Elevation>) {
    for problem in &report.incompatible {
        println!("{} {problem}", output::red("Cannot run here:"));
    }
    if report.missing.is_empty() {
        return;
    }
    println!("{}", output::bold("Missing system libraries"));
    let width = report
        .missing
        .iter()
        .map(|missing| missing.soname.len())
        .max()
        .unwrap_or(0);
    for missing in &report.missing {
        let package = missing
            .package
            .as_deref()
            .map_or_else(|| output::dim("(package not found)"), str::to_string);
        println!(
            "  {:<width$}  {:<22} {}",
            missing.soname,
            package,
            output::dim(&format!("needed by {}", missing.needed_by.join(", ")))
        );
    }
    println!();
    let packages = report.packages();
    let without_package: Vec<&str> = report
        .missing
        .iter()
        .filter(|missing| missing.package.is_none())
        .map(|missing| missing.soname.as_str())
        .collect();
    if let (Some(manager), false) = (manager, packages.is_empty()) {
        let step = installer::install_packages(manager, &packages);
        println!(
            "Install them first with:\n  {}",
            step.command_line(elevation)
        );
        println!(
            "or run this command again with {}.",
            output::bold("--install-deps")
        );
    }
    if let Some(hint) = &report.hint {
        println!("{}", output::yellow(hint));
    } else if packages.is_empty() {
        println!(
            "{}",
            output::yellow("Glori could not find which packages provide them.")
        );
    } else if !without_package.is_empty() {
        println!(
            "{}",
            output::yellow(&format!(
                "No package was found for {}. It may come from a repository Glori does not search.",
                without_package.join(", ")
            ))
        );
    }
    println!();
}

pub fn question(report: &Report, deps_will_be_installed: bool, base: &'static str) -> &'static str {
    if base != "Proceed?" {
        base
    } else if !report.incompatible.is_empty() {
        "This program was built for another CPU architecture and will not run here. Install anyway?"
    } else if !report.missing.is_empty() && !deps_will_be_installed {
        "Some libraries are missing, so the program may not start. Install anyway?"
    } else {
        base
    }
}

#[cfg(test)]
mod tests {
    use glori_core::libs::Missing;

    use super::*;

    fn report(missing: bool, incompatible: bool) -> Report {
        Report {
            missing: if missing {
                vec![Missing {
                    soname: "libx.so.1".into(),
                    needed_by: vec!["app".into()],
                    package: Some("x".into()),
                }]
            } else {
                vec![]
            },
            incompatible: if incompatible {
                vec!["bin/app is built for aarch64".into()]
            } else {
                vec![]
            },
            scanned: 1,
            hint: None,
        }
    }

    #[test]
    fn nothing_wrong_keeps_the_normal_question() {
        assert_eq!(
            question(&report(false, false), false, "Proceed?"),
            "Proceed?"
        );
    }

    #[test]
    fn missing_libraries_change_the_question_unless_they_will_be_installed() {
        let missing = report(true, false);
        assert!(question(&missing, false, "Proceed?").contains("libraries are missing"));
        assert_eq!(question(&missing, true, "Proceed?"), "Proceed?");
    }

    #[test]
    fn a_wrong_architecture_wins_over_missing_libraries() {
        assert!(question(&report(true, true), true, "Proceed?").contains("CPU architecture"));
    }

    #[test]
    fn a_trust_question_is_never_overridden() {
        let both = report(true, true);
        assert_eq!(
            question(
                &both,
                false,
                "This package is not signed. Install it anyway?"
            ),
            "This package is not signed. Install it anyway?"
        );
    }
}

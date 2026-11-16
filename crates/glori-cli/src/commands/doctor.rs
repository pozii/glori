use std::process::ExitCode;

use glori_core::{distro, doctor};

use crate::output;

pub fn run(json: bool) -> anyhow::Result<ExitCode> {
    let report = doctor::run(distro::detect()?);
    let healthy = report.healthy();
    if json {
        output::json(&report)?;
    } else {
        print_report(&report);
    }
    Ok(if healthy {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    })
}

fn print_report(report: &doctor::Report) {
    println!(
        "{} {} ({} family)\n",
        output::bold("Distribution"),
        report.distro.name,
        report.distro.family.label()
    );
    for check in &report.checks {
        let (mark, location) = match (&check.path, check.required) {
            (Some(path), _) => (output::green("✓"), path.display().to_string()),
            (None, true) => (output::red("✗"), "not found".to_string()),
            (None, false) => (output::yellow("○"), "not found (optional)".to_string()),
        };
        println!(
            "{mark} {:<8} {} {}",
            check.tool,
            output::dim(check.purpose),
            output::dim(&format!("- {location}"))
        );
    }
    println!();
    if report.healthy() {
        println!("{}", output::green("All required tools are available."));
    } else {
        println!("{}", output::red("Some required tools are missing."));
    }
}

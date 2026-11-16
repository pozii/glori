use std::path::Path;

use glori_core::{Distro, Format, Strategy, distro, format, plan};
use serde::Serialize;

use crate::output;

#[derive(Serialize)]
struct Report<'a> {
    file: &'a Path,
    format: Format,
    distro: Distro,
    strategy: Strategy,
}

pub fn run(file: &Path, json: bool) -> anyhow::Result<()> {
    let format = format::detect(file)?;
    let distro = distro::detect()?;
    let strategy = plan::plan(format, &distro);
    let report = Report {
        file,
        format,
        distro,
        strategy,
    };
    if json {
        return output::json(&report);
    }
    print_report(&report);
    Ok(())
}

fn print_report(report: &Report) {
    let rows = [
        ("File", report.file.display().to_string()),
        ("Format", report.format.label().to_string()),
        (
            "Distro",
            format!(
                "{} ({} family)",
                report.distro.name,
                report.distro.family.label()
            ),
        ),
        ("Plan", describe(report.strategy)),
    ];
    for (label, value) in rows {
        println!("{} {value}", output::bold(&format!("{label:<8}")));
    }
}

fn describe(strategy: Strategy) -> String {
    match strategy {
        Strategy::Native { manager } => {
            output::green(&format!("install natively with {}", manager.binary()))
        }
        Strategy::Convert => output::yellow("not native to this distribution, conversion needed"),
        Strategy::Archive => output::green("extract and register in your user directory"),
        Strategy::AppImage => output::green("install as an AppImage with a menu entry"),
    }
}

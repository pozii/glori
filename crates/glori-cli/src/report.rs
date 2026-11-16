use std::path::Path;

use glori_core::{Distro, Format, Strategy, Trust, distro, format, plan, signature};
use serde::Serialize;

use crate::output;

#[derive(Serialize)]
pub struct Summary<'a> {
    pub file: &'a Path,
    pub format: Format,
    pub distro: Distro,
    pub strategy: Strategy,
    pub signature: Trust,
}

impl<'a> Summary<'a> {
    pub fn detect(file: &'a Path) -> anyhow::Result<Self> {
        let format = format::detect(file)?;
        let distro = distro::detect()?;
        let strategy = plan::plan(format, &distro);
        let signature = signature::check(format, file);
        Ok(Summary {
            file,
            format,
            distro,
            strategy,
            signature,
        })
    }

    pub fn print(&self) {
        let rows = [
            ("File", self.file.display().to_string()),
            ("Format", self.format.label().to_string()),
            (
                "Distro",
                format!(
                    "{} ({} family)",
                    self.distro.name,
                    self.distro.family.label()
                ),
            ),
            ("Plan", describe(self.strategy)),
            ("Signature", paint(self.signature)),
        ];
        for (label, value) in rows {
            println!("{} {value}", output::bold(&format!("{label:<10}")));
        }
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

fn paint(trust: Trust) -> String {
    match trust {
        Trust::Verified => output::green(trust.label()),
        Trust::Unsigned => output::yellow(trust.label()),
        Trust::Invalid => output::red(trust.label()),
        Trust::Unverifiable => output::dim(trust.label()),
    }
}

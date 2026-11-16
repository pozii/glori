use std::path::Path;

use glori_core::{Distro, Format, Strategy, distro, format, plan};
use serde::Serialize;

use crate::output;

#[derive(Serialize)]
pub struct Summary<'a> {
    pub file: &'a Path,
    pub format: Format,
    pub distro: Distro,
    pub strategy: Strategy,
}

impl<'a> Summary<'a> {
    pub fn detect(file: &'a Path) -> anyhow::Result<Self> {
        let format = format::detect(file)?;
        let distro = distro::detect()?;
        let strategy = plan::plan(format, &distro);
        Ok(Summary {
            file,
            format,
            distro,
            strategy,
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
        ];
        for (label, value) in rows {
            println!("{} {value}", output::bold(&format!("{label:<8}")));
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

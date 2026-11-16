use std::{env, fs};

use anyhow::{Context, bail};
use glori_core::{
    Format, Trust,
    download::Progress,
    update::{self, Origin, Release, Status, Version},
    verify,
};
use serde::Serialize;

use crate::{output, prompt};

const UPDATE_URL_VAR: &str = "GLORI_UPDATE_URL";

pub struct Options {
    pub check: bool,
    pub yes: bool,
    pub json: bool,
}

#[derive(Serialize)]
struct Report<'a> {
    current: &'a Version,
    latest: &'a Version,
    update_available: bool,
}

pub fn run(options: Options) -> anyhow::Result<()> {
    let Options { check, yes, json } = options;
    let exe = env::current_exe().context("cannot locate the running glori binary")?;
    let exe = fs::canonicalize(&exe).unwrap_or(exe);
    if update::origin_of(&exe) == Origin::SystemPackage && !check {
        bail!(
            "{} belongs to your system; update it with your package manager instead",
            exe.display()
        );
    }
    let current = Version::current();
    let url = env::var(UPDATE_URL_VAR).unwrap_or_else(|_| update::RELEASES_URL.to_string());
    let status = update::check(&current, &url)?;
    let latest = match &status {
        Status::Available(release) => &release.version,
        Status::UpToDate => &current,
    };
    if json {
        return output::json(&Report {
            current: &current,
            latest,
            update_available: matches!(status, Status::Available(_)),
        });
    }
    let Status::Available(release) = status else {
        println!("Glori {current} is up to date.");
        return Ok(());
    };
    println!("{} {current}", output::bold("Current"));
    println!("{} {}", output::bold("Latest "), release.version);
    if check {
        println!("Run `glori self-update` to install it.");
        return Ok(());
    }
    if !yes && !prompt::confirm(&format!("Update Glori to {}?", release.version))? {
        bail!("cancelled, nothing was changed");
    }
    apply(&release, &exe)?;
    println!(
        "{}",
        output::green(&format!("Updated Glori to {}.", release.version))
    );
    Ok(())
}

fn apply(release: &Release, exe: &std::path::Path) -> anyhow::Result<()> {
    let download = update::fetch(release, env::consts::ARCH, Progress::Show)?;
    let verdict = verify::check(Format::TarGz, download.path(), &verify::Options::default());
    if verdict.trust != Trust::Verified {
        bail!(
            "refusing to update: the download could not be verified ({})",
            verdict
                .detail
                .unwrap_or_else(|| verdict.trust.label().to_string())
        );
    }
    let staging = tempfile::Builder::new().prefix("glori-update-").tempdir()?;
    let binary = staging.path().join("glori");
    update::extract_binary(download.path(), &binary)?;
    update::replace(exe, &binary, &release.version.to_string())?;
    Ok(())
}

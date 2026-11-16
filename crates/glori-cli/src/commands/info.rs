use std::path::Path;

use glori_core::verify;

use crate::{output, report::Summary};

pub fn run(file: &Path, sha256: Option<String>, json: bool) -> anyhow::Result<()> {
    let summary = Summary::detect(file, &verify::Options { sha256 })?;
    if json {
        return output::json(&summary);
    }
    summary.print();
    Ok(())
}

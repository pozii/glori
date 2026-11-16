use std::path::Path;

use crate::{output, report::Summary};

pub fn run(file: &Path, json: bool) -> anyhow::Result<()> {
    let summary = Summary::detect(file)?;
    if json {
        return output::json(&summary);
    }
    summary.print();
    Ok(())
}

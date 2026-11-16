use glori_core::verify;

use super::source;
use crate::{output, report::Summary};

pub fn run(input: &str, sha256: Option<String>, json: bool) -> anyhow::Result<()> {
    let source = source::resolve(input, sha256.is_some(), json)?;
    let summary = Summary::detect(source.path(), &verify::Options { sha256 })?;
    if json {
        return output::json(&summary);
    }
    summary.print();
    Ok(())
}

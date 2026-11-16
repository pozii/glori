use glori_core::{Elevation, Operation, Paths, runner};

use crate::output;

pub fn print_operations(operations: &[Operation], elevation: &glori_core::Result<Elevation>) {
    println!("\n{}", output::bold("Steps"));
    let shown = elevation.as_ref().ok().copied();
    for (index, operation) in operations.iter().enumerate() {
        println!("  {}. {}", index + 1, operation.describe(shown));
    }
    if elevation.is_err() && operations.iter().any(Operation::privileged) {
        println!(
            "{}",
            output::yellow("  root is required but neither sudo nor pkexec was found")
        );
    }
    println!();
}

pub fn needs_elevation(
    operations: &[Operation],
    detected: glori_core::Result<Elevation>,
) -> anyhow::Result<Elevation> {
    if operations.iter().any(Operation::privileged) {
        Ok(detected?)
    } else {
        Ok(detected.unwrap_or(Elevation::Direct))
    }
}

pub fn execute(operations: &[Operation], elevation: Elevation, paths: Paths) -> anyhow::Result<()> {
    let context = runner::Context { elevation, paths };
    for (index, operation) in operations.iter().enumerate() {
        println!(
            "{} {}",
            output::bold(&format!("[{}/{}]", index + 1, operations.len())),
            operation.describe(Some(elevation))
        );
        runner::apply(operation, &context)?;
    }
    println!("{}", output::green("Done."));
    Ok(())
}

use std::process::Command;

use crate::{
    error::{Error, Result},
    privilege::Elevation,
    step::Step,
};

pub fn run(step: &Step, elevation: Elevation) -> Result<()> {
    let command = step.command_line(Some(elevation));
    let argv = step.argv(Some(elevation));
    let status = Command::new(argv[0])
        .args(&argv[1..])
        .status()
        .map_err(|source| Error::Spawn {
            command: command.clone(),
            source,
        })?;
    if status.success() {
        Ok(())
    } else {
        Err(Error::StepFailed { command, status })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn successful_step() {
        assert!(run(&Step::new("true", &[], false), Elevation::Direct).is_ok());
    }

    #[test]
    fn failing_step_reports_the_command() {
        let error = run(&Step::new("false", &[], false), Elevation::Direct).unwrap_err();
        assert!(matches!(error, Error::StepFailed { ref command, .. } if command == "false"));
    }

    #[test]
    fn missing_program_is_a_spawn_error() {
        let step = Step::new("glori-no-such-program", &[], false);
        assert!(matches!(
            run(&step, Elevation::Direct),
            Err(Error::Spawn { .. })
        ));
    }
}

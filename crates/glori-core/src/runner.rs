use std::process::Command;

use crate::{
    error::{Error, Result},
    fsops,
    operation::Operation,
    paths::Paths,
    privilege::Elevation,
    registry::Registry,
    step::Step,
};

pub struct Context {
    pub elevation: Elevation,
    pub paths: Paths,
}

pub fn apply(operation: &Operation, context: &Context) -> Result<()> {
    match operation {
        Operation::Run(step) => run_step(step, context.elevation),
        Operation::Extract {
            archive,
            format,
            dest,
            flatten,
        } => fsops::extract(archive, *format, dest, *flatten),
        Operation::Copy { from, to, mode } => fsops::copy(from, to, *mode),
        Operation::Link { target, link } => fsops::link(target, link),
        Operation::Write {
            path,
            content,
            mode,
        } => fsops::write(path, content, *mode),
        Operation::Remove { path } => fsops::remove(path, &context.paths),
        Operation::Register(entry) => {
            let file = context.paths.registry_file();
            let mut registry = Registry::load(&file)?;
            registry.insert(entry.clone())?;
            registry.save(&file)
        }
        Operation::Unregister { name } => {
            let file = context.paths.registry_file();
            let mut registry = Registry::load(&file)?;
            registry.remove(name);
            registry.save(&file)
        }
    }
}

pub fn run_step(step: &Step, elevation: Elevation) -> Result<()> {
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
    use crate::registry::{Entry, Kind};

    fn context(home: &std::path::Path) -> Context {
        Context {
            elevation: Elevation::Direct,
            paths: Paths::new(home.to_path_buf(), home.join(".local/share")),
        }
    }

    fn entry() -> Entry {
        Entry {
            name: "a".into(),
            version: None,
            kind: Kind::Archive,
            installed_at: 1,
            source: "a.tar.gz".into(),
            files: vec![],
        }
    }

    #[test]
    fn successful_step() {
        assert!(run_step(&Step::new("true", &[], false), Elevation::Direct).is_ok());
    }

    #[test]
    fn failing_step_reports_the_command() {
        let error = run_step(&Step::new("false", &[], false), Elevation::Direct).unwrap_err();
        assert!(matches!(error, Error::StepFailed { ref command, .. } if command == "false"));
    }

    #[test]
    fn missing_program_is_a_spawn_error() {
        let step = Step::new("glori-no-such-program", &[], false);
        assert!(matches!(
            run_step(&step, Elevation::Direct),
            Err(Error::Spawn { .. })
        ));
    }

    #[test]
    fn register_then_unregister_updates_the_registry_file() {
        let dir = tempfile::tempdir().unwrap();
        let context = context(dir.path());
        apply(&Operation::Register(entry()), &context).unwrap();
        let file = context.paths.registry_file();
        assert!(Registry::load(&file).unwrap().find("a").is_some());
        assert!(matches!(
            apply(&Operation::Register(entry()), &context),
            Err(Error::AlreadyInstalled(_))
        ));
        apply(&Operation::Unregister { name: "a".into() }, &context).unwrap();
        assert!(Registry::load(&file).unwrap().find("a").is_none());
    }
}

use std::path::PathBuf;

use serde::Serialize;

use crate::{format::Format, privilege::Elevation, registry::Entry, step::Step};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum Operation {
    Run(Step),
    Extract {
        archive: PathBuf,
        format: Format,
        dest: PathBuf,
        flatten: bool,
    },
    Copy {
        from: PathBuf,
        to: PathBuf,
        mode: u32,
    },
    Link {
        target: PathBuf,
        link: PathBuf,
    },
    Write {
        path: PathBuf,
        content: String,
        mode: u32,
    },
    Register(Entry),
    Remove {
        path: PathBuf,
    },
    Unregister {
        name: String,
    },
}

impl Operation {
    pub fn privileged(&self) -> bool {
        matches!(self, Operation::Run(step) if step.privileged)
    }

    pub fn describe(&self, elevation: Option<Elevation>) -> String {
        match self {
            Operation::Run(step) => step.command_line(elevation),
            Operation::Extract { archive, dest, .. } => {
                format!("extract {} into {}", archive.display(), dest.display())
            }
            Operation::Copy { from, to, .. } => {
                format!("copy {} to {}", from.display(), to.display())
            }
            Operation::Link { target, link } => {
                format!("link {} -> {}", link.display(), target.display())
            }
            Operation::Write { path, .. } => format!("write {}", path.display()),
            Operation::Register(entry) => format!("record {} in the Glori registry", entry.name),
            Operation::Remove { path } => format!("remove {}", path.display()),
            Operation::Unregister { name } => format!("forget {name} in the Glori registry"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_privileged_commands_need_elevation() {
        assert!(Operation::Run(Step::new("dnf", &[], true)).privileged());
        assert!(!Operation::Run(Step::new("tar", &[], false)).privileged());
        assert!(!Operation::Remove { path: "/x".into() }.privileged());
    }

    #[test]
    fn descriptions_are_readable() {
        let link = Operation::Link {
            target: "/a/b".into(),
            link: "/c".into(),
        };
        assert_eq!(link.describe(None), "link /c -> /a/b");
        let run = Operation::Run(Step::new("dnf", &["install"], true));
        assert_eq!(run.describe(Some(Elevation::Sudo)), "sudo dnf install");
    }

    #[test]
    fn operations_serialize_with_a_tag() {
        let json = serde_json::to_value(Operation::Remove { path: "/x".into() }).unwrap();
        assert_eq!(json["op"], "remove");
        let run = serde_json::to_value(Operation::Run(Step::new("dnf", &["i"], true))).unwrap();
        assert_eq!(run["op"], "run");
        assert_eq!(run["program"], "dnf");
    }
}

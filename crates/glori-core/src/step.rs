use serde::Serialize;

use crate::privilege::Elevation;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Step {
    pub program: String,
    pub args: Vec<String>,
    pub privileged: bool,
}

impl Step {
    pub fn new(program: &str, args: &[&str], privileged: bool) -> Self {
        Step {
            program: program.to_string(),
            args: args.iter().map(|arg| arg.to_string()).collect(),
            privileged,
        }
    }

    pub fn argv(&self, elevation: Option<Elevation>) -> Vec<&str> {
        let prefix = elevation
            .filter(|_| self.privileged)
            .map_or(&[][..], Elevation::prefix);
        prefix
            .iter()
            .copied()
            .chain([self.program.as_str()])
            .chain(self.args.iter().map(String::as_str))
            .collect()
    }

    pub fn command_line(&self, elevation: Option<Elevation>) -> String {
        self.argv(elevation)
            .into_iter()
            .map(quote)
            .collect::<Vec<_>>()
            .join(" ")
    }
}

fn quote(arg: &str) -> String {
    let plain = !arg.is_empty()
        && arg
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "-_./=:@+,".contains(c));
    if plain {
        arg.to_string()
    } else {
        format!("'{}'", arg.replace('\'', "'\\''"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn privileged_steps_get_the_prefix() {
        let step = Step::new("dnf", &["install", "-y", "/tmp/a.rpm"], true);
        assert_eq!(
            step.command_line(Some(Elevation::Sudo)),
            "sudo dnf install -y /tmp/a.rpm"
        );
    }

    #[test]
    fn unprivileged_steps_never_get_the_prefix() {
        let step = Step::new("tar", &["-xf", "a.tar"], false);
        assert_eq!(step.command_line(Some(Elevation::Sudo)), "tar -xf a.tar");
    }

    #[test]
    fn direct_elevation_adds_nothing() {
        let step = Step::new("dnf", &["install"], true);
        assert_eq!(step.command_line(Some(Elevation::Direct)), "dnf install");
        assert_eq!(step.command_line(None), "dnf install");
    }

    #[test]
    fn awkward_arguments_are_quoted() {
        let step = Step::new("dnf", &["install", "/tmp/my app's.rpm"], false);
        assert_eq!(
            step.command_line(None),
            "dnf install '/tmp/my app'\\''s.rpm'"
        );
    }
}

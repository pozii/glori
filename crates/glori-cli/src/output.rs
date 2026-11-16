use owo_colors::{OwoColorize, Stream};
use serde::Serialize;

pub fn json<T: Serialize>(value: &T) -> anyhow::Result<()> {
    println!("{}", serde_json::to_string_pretty(value)?);
    Ok(())
}

pub fn green(text: &str) -> String {
    text.if_supports_color(Stream::Stdout, |t| t.green())
        .to_string()
}

pub fn yellow(text: &str) -> String {
    text.if_supports_color(Stream::Stdout, |t| t.yellow())
        .to_string()
}

pub fn red(text: &str) -> String {
    text.if_supports_color(Stream::Stdout, |t| t.red())
        .to_string()
}

pub fn bold(text: &str) -> String {
    text.if_supports_color(Stream::Stdout, |t| t.bold())
        .to_string()
}

pub fn dim(text: &str) -> String {
    text.if_supports_color(Stream::Stdout, |t| t.dimmed())
        .to_string()
}

pub fn error(error: &anyhow::Error) {
    let label = "error:".if_supports_color(Stream::Stderr, |t| t.red().bold().to_string());
    eprintln!("{label} {error:#}");
}

# Contributing to Glori

Thanks for helping. Glori is Apache-2.0 licensed; by contributing you agree your work is released under the same license.

## Getting started

```
git clone https://github.com/pozii/glori.git
cd glori
cargo build
cargo test
```

## Before opening a pull request

```
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

## Code rules

- Code and all user-facing text are in English.
- Write the simplest, most efficient code that works.
- No code comments. Express intent through clear names and small functions.
- Keep code modular: one responsibility per module.
- New formats, sources and distributions are added as new modules behind the existing traits, without changing the core.

## Commits and PRs

- Small, focused changes with a clear description.
- Add or update tests for behavior changes.

## Ways to help

- Add a format installer or distro support.
- Write recipes for tricky archives.
- Test on distributions you use and report results.
- Improve documentation.

## Security

Report vulnerabilities privately as described in [SECURITY.md](SECURITY.md).

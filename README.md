# Glori

Install software the same way on every Linux distribution.

```
glori install ./app.deb
glori install ./app.tar.gz
glori install ./App.AppImage
glori install https://example.com/app.rpm
```

## Why

Downloaded Linux software comes in many shapes: `.deb`, `.rpm`, `.tar.gz`, AppImage. Each one needs a different tool, different steps and different cleanup. Glori turns that into one command.

## Principles

- **Universal** - format and distribution differences are not the user's problem.
- **Transparent** - shows the plan first, then acts.
- **Reversible** - records everything it installs and removes it cleanly.
- **User-owned** - no root unless it is required.
- **Complementary** - wraps `apt`, `dnf`, `pacman`; never replaces them.
- **Extensible** - small core, community-driven formats, sources and recipes.

## Status

Early development, no release yet. Working today: `glori info`, `glori doctor` and shell completions. Installing and removing software is next.

## Planned commands

| Command | Purpose |
|---|---|
| `glori install <file\|url>` | Install a package or archive |
| `glori remove <name>` | Remove an installed item |
| `glori list` | List what Glori installed |
| `glori info <file>` | Show format, distro and the install plan |
| `glori doctor` | Check the system and required tools |

Global flags: `--dry-run`, `--yes`, `--system`, `--json`, `-q`, `-v`, `--no-color`.

## Architecture

Cargo workspace with a reusable core and thin frontends.

- `glori-core` - format detection, distro detection, installers, registry
- `glori-cli` - the `glori` command
- `glori-gui` - planned, built on the same core

Extension points are traits: `Installer` (formats), `Source` (where files come from), `Distro` (package managers).

## Support

Glori is free and open source. If it saves you time, you can support its development on [Buy Me a Coffee](https://buymeacoffee.com/pozii).

## Contributing

Contributions are welcome. See [CONTRIBUTING.md](CONTRIBUTING.md).

## License

Apache License 2.0. See [LICENSE](LICENSE).

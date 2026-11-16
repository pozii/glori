# Glori

Install software the same way on every Linux distribution.

```
glori install ./app.rpm
glori install ./tool-1.2-linux-x64.tar.gz
glori install ./App.AppImage
glori list
glori remove tool
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

Early development, no release yet. Working today:

- `glori install` for packages that belong to your distribution (`.deb` on Debian-based systems, `.rpm` on Fedora and openSUSE, Arch packages on Arch), with RPM signature checks
- `glori install` for tarballs (`.tar.gz`, `.tar.xz`, `.tar.zst`, `.tar.bz2`) and AppImages, entirely in your home directory, no root needed
- `glori list` and `glori remove` for everything installed that way
- `glori info`, `glori doctor` and shell completions

Not done yet: installing a package from another distribution family (for example a `.deb` on Fedora), downloading from a URL, extracting icons from AppImages, system-wide installs, and removing packages that were installed through the system package manager.

## Commands

| Command | Purpose |
|---|---|
| `glori install <file>` | Install a package, archive or AppImage (`--dry-run`, `--yes`, `--name <name>`, `--allow-untrusted`) |
| `glori remove <name>` | Remove something Glori installed (`--dry-run`, `--yes`) |
| `glori list` | List what Glori installed |
| `glori info <file>` | Show format, distribution, signature state and the install plan |
| `glori doctor` | Check the system and required tools |
| `glori completions <shell>` | Generate shell completions |

Global flags: `--json` (for `info`, `doctor`, `list` and `--dry-run` plans) and `--no-color`.

## Where things go

Tarballs and AppImages are installed per user:

| What | Where |
|---|---|
| Application files | `~/.local/opt/<name>/` |
| Commands | symlinks in `~/.local/bin/` |
| Menu entry (graphical apps) | `~/.local/share/applications/<name>.desktop` |
| Glori's record of what it installed | `~/.local/share/glori/registry.json` |

The name and version come from the file name (`ripgrep-15.2.0-x86_64-unknown-linux-musl.tar.gz` becomes `ripgrep` 15.2.0); use `--name` to choose another. A tarball gets a menu entry only if it ships an icon or a `.desktop` file, so command line tools do not clutter your menu. Make sure `~/.local/bin` is in your `PATH`.

## Safety

- Glori never overwrites a file that already exists, and `remove` only deletes paths inside the directories above, even if the registry was edited by hand.
- Archives with absolute paths, `..` components or symlinks that lead out of the install directory are rejected before anything is written.
- Extraction happens in a temporary directory and is moved into place only on success, so a broken or truncated download leaves nothing behind.
- RPM signatures are verified with `rpm -K`. A failed check blocks the install, and an unsigned package needs `--allow-untrusted` in non-interactive use. Other formats carry no embedded signature and are reported as not checkable for now.
- Every command can show its plan first with `--dry-run`.

## Architecture

Cargo workspace with a reusable core and thin frontends.

- `glori-core` - format and distribution detection, installers, archive handling, registry
- `glori-cli` - the `glori` command
- `glori-gui` - planned, built on the same core

Installing is a list of operations: run a command, extract, copy, link, write a file, update the registry. An `Installer` turns a file into operations, the CLI shows them, and only then are they applied. Archive decoding uses pure Rust crates so the binary can be built fully static with musl.

## Support

Glori is free and open source. If it saves you time, you can support its development on [Buy Me a Coffee](https://buymeacoffee.com/pozii).

## Contributing

Contributions are welcome. See [CONTRIBUTING.md](CONTRIBUTING.md).

## License

Apache License 2.0. See [LICENSE](LICENSE).

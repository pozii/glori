<p align="center">
  <a href="https://github.com/pozii/glori">
    <picture>
      <source srcset=".github/assets/logo-dark.svg" media="(prefers-color-scheme: dark)">
      <source srcset=".github/assets/logo-light.svg" media="(prefers-color-scheme: light)">
      <img src=".github/assets/logo-light.svg" alt="Glori" height="96">
    </picture>
  </a>
</p>
<p align="center">Install software the same way on every Linux distribution.</p>
<p align="center">
  <a href="https://github.com/pozii/glori/releases"><img alt="Release" src="https://img.shields.io/github/v/release/pozii/glori?include_prereleases&style=flat-square" /></a>
  <a href="https://github.com/pozii/glori/actions/workflows/ci.yml"><img alt="Build status" src="https://img.shields.io/github/actions/workflow/status/pozii/glori/ci.yml?style=flat-square&branch=main" /></a>
  <a href="LICENSE"><img alt="License" src="https://img.shields.io/github/license/pozii/glori?style=flat-square" /></a>
  <img alt="Written in Rust" src="https://img.shields.io/badge/rust-1.87%2B-orange?style=flat-square" />
</p>

```console
$ glori install https://github.com/BurntSushi/ripgrep/releases/download/15.1.0/ripgrep-15.1.0-x86_64-unknown-linux-musl.tar.gz --dry-run
File          /tmp/glori-download-0CrACa/ripgrep-15.1.0-x86_64-unknown-linux-musl.tar.gz
Format        gzip tarball (.tar.gz)
Distro        Fedora Linux 45 (Budgie) (Fedora family)
Plan          extract and register in your user directory
Verification  verified (sha256 matches ripgrep-15.1.0-x86_64-unknown-linux-musl.tar.gz.sha256)

Steps
  1. extract ... into ~/.local/opt/ripgrep
  2. link ~/.local/bin/rg -> ~/.local/opt/ripgrep/rg
  3. link ~/.local/share/man/man1/rg.1 -> ~/.local/opt/ripgrep/doc/rg.1
  4. link ~/.local/share/fish/completions/rg.fish -> ~/.local/opt/ripgrep/complete/rg.fish
  5. link ~/.local/share/bash-completion/completions/rg -> ~/.local/opt/ripgrep/complete/rg.bash
  6. record ripgrep in the Glori registry

Dry run: nothing was changed.
```

---

### Installation

```bash
# YOLO
curl -fsSL https://raw.githubusercontent.com/pozii/glori/main/install.sh | sh

# From source
cargo install --git https://github.com/pozii/glori glori-cli
```

The script puts a static `glori` binary in `~/.local/bin` after checking its SHA-256. No root needed.

Every [release](https://github.com/pozii/glori/releases) also ships distribution packages for x86_64 and aarch64:

| Distribution | Package |
| --- | --- |
| Debian, Ubuntu and derivatives | `glori_<version>_<arch>.deb` |
| Fedora, openSUSE and derivatives | `glori-<version>-1.<arch>.rpm` |
| Arch Linux | [`packaging/arch/PKGBUILD`](packaging/arch/PKGBUILD) |
| Anything else | `glori-<version>-<arch>-unknown-linux-musl.tar.gz` |

> [!TIP]
> Update with `glori self-update` (or `--check` to only look). If Glori came from a `.deb`, `.rpm` or Arch package, update it with the package manager instead.

Releases use calendar versions: `2026.11.17`, and `2026.11.17.1` for a second release the same day. See [RELEASING.md](RELEASING.md).

### Usage

```bash
glori install ./app.rpm
glori install ./tool-1.2-linux-x64.tar.gz
glori install https://example.com/tool-1.2-linux-x64.tar.gz
glori install ./App.AppImage
glori list
glori remove tool
```

Every install shows its plan first, and `--dry-run` stops there.

### What it handles

| Format | How it is installed | Root |
| --- | --- | --- |
| `.deb` | `apt-get` on Debian and derivatives | yes |
| `.rpm` | `dnf` on Fedora, `zypper` on openSUSE, signatures checked with `rpm -K` | yes |
| Arch package (`.pkg.tar.zst`) | `pacman` on Arch and derivatives | yes |
| `.tar.gz`, `.tar.xz`, `.tar.zst`, `.tar.bz2` | extracted into `~/.local/opt`, commands linked into `~/.local/bin`, missing system libraries reported | no |
| AppImage | copied into `~/.local/opt` with a menu entry | no |

Installing a package from another distribution family (a `.deb` on Fedora) is planned, not done.

### Principles

- **Universal** - format and distribution differences are not your problem.
- **Transparent** - shows the plan first, then acts.
- **Reversible** - records everything it installs and removes it cleanly.
- **User-owned** - no root unless it is required.
- **Complementary** - wraps `apt`, `dnf`, `pacman`; never replaces them.
- **Verified** - checks SHA-256 sums and GPG or RPM signatures before it installs.

### Status

Early development, the first release is not out yet. Working today: native installs, tarballs and AppImages, URL downloads, checksum and signature verification, missing library detection with `--install-deps`, `list`, `remove` and `self-update`.

Not done yet: cross-distribution conversion, AppImage icons, system-wide installs of tarballs, removing packages that were installed through the system package manager, and the GUI.

---

### Commands

| Command | Purpose |
|---|---|
| `glori install <file or URL>` | Install a package, archive or AppImage from a path or an http(s) URL (`--dry-run`, `--yes`, `--name <name>`, `--sha256 <hash>`, `--install-deps`, `--allow-untrusted`) |
| `glori remove <name>` | Remove something Glori installed (`--dry-run`, `--yes`) |
| `glori self-update` | Update Glori from the latest release (`--check`, `--yes`) |
| `glori list` | List what Glori installed |
| `glori info <file>` | Show format, distribution, verification result and the install plan (`--sha256 <hash>`) |
| `glori doctor` | Check the system and required tools |
| `glori completions <shell>` | Generate shell completions |

Global flags: `--json` (for `info`, `doctor`, `list` and `--dry-run` plans) and `--no-color`.

### Where things go

Tarballs and AppImages are installed per user:

| What | Where |
|---|---|
| Application files | `~/.local/opt/<name>/` |
| Commands | symlinks in `~/.local/bin/` |
| Menu entry (graphical apps) | `~/.local/share/applications/<name>.desktop` |
| Man pages | links in `~/.local/share/man/man<N>/` |
| Shell completions | links in `~/.local/share/bash-completion/completions/` and `~/.local/share/fish/completions/` |
| Glori's record of what it installed | `~/.local/share/glori/registry.json` |

The name and version come from the file name (`ripgrep-15.2.0-x86_64-unknown-linux-musl.tar.gz` becomes `ripgrep` 15.2.0); use `--name` to choose another. A tarball gets a menu entry only if it ships an icon or a `.desktop` file, so command line tools do not clutter your menu. Make sure `~/.local/bin` is in your `PATH`.

### Safety

- Glori never overwrites a file that already exists, and `remove` only deletes paths inside the directories above, even if the registry was edited by hand.
- Archives with absolute paths, `..` components or symlinks that lead out of the install directory are rejected before anything is written.
- Extraction happens in a temporary directory and is moved into place only on success, so a broken or truncated download leaves nothing behind.
- Downloads are checked when there is something to check them against (see below). A failed check blocks the install, and `--allow-untrusted` is needed to go ahead anyway.
- Every command can show its plan first with `--dry-run`.

### Missing system libraries

A tarball often needs libraries that are not in the archive. Glori reads the dynamic dependencies of every program and library inside it before installing, and compares them with what is bundled in the archive and what your system has:

```
Missing system libraries
  libcrypt.so.1        libxcrypt-compat   needed by libfilezilla.so.61
  libgdk-x11-2.0.so.0  gtk2               needed by filezilla, libwx_gtk2u_core-3.2.so.0

Install them first with:
  sudo dnf install -y gtk2 libxcrypt-compat
or run this command again with --install-deps.
```

`--install-deps` runs that package install as the first step of the plan. Package names come from the distribution's own tools: `dnf repoquery`, `zypper`, `apt-file` (Debian and Ubuntu, `apt install apt-file && apt-file update`) and `pacman -F` (Arch, `pacman -Fy`). Where a lookup tool is missing or out of date Glori tells you what to run, and libraries it cannot map to a package are listed without one. A program built for another CPU architecture is reported as `Cannot run here`. Missing libraries are a warning, not a block: some programs start without them.

This check applies to tarballs. Native packages already have their dependencies resolved by the package manager, and AppImages bundle theirs.

### Verifying downloads

`glori info` and `glori install` look for evidence next to the file and show the result in the `Verification` row:

| Evidence | How it is used |
|---|---|
| `--sha256 <hash>` | You provide the hash you got from a source you trust. Always checked first. |
| `<file>.sha256`, `SHA256SUMS` | Found automatically. GNU, BSD and bare-hash formats are understood. |
| `<file>.asc`, `<file>.sig` | Checked with `gpg` against your own keyring. A signature from a key you do not have is a failure, not a pass. |
| RPM signature | Checked with `rpm -K`. An unsigned RPM needs `--allow-untrusted` in non-interactive use. |

A checksum file served from the same place as the download proves the file arrived intact, not who published it; only a signature or a hash you obtained separately proves that. When there is nothing to check against, Glori says so and installs normally, since most downloads ship no checksum.

### Architecture

Cargo workspace with a reusable core and thin frontends.

- `glori-core` - format and distribution detection, installers, archive handling, registry
- `glori-cli` - the `glori` command
- `glori-gui` - planned, built on the same core

Installing is a list of operations: run a command, extract, copy, link, write a file, update the registry. An `Installer` turns a file into operations, the CLI shows them, and only then are they applied. Archive decoding uses pure Rust crates so the binary can be built fully static with musl.

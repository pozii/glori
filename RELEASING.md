# Releasing

Releases are built by `.github/workflows/release.yml` when a `v*` tag is pushed.

1. Set the new version in `[workspace.package]` in `Cargo.toml` and run `cargo build` so `Cargo.lock` follows.
2. Commit, then tag and push: `git tag vX.Y.Z && git push origin main vX.Y.Z`.
3. The workflow refuses to run if the tag and the crate version differ, tests and builds static musl binaries for x86_64 and aarch64, and publishes a GitHub release with:
   - `glori-X.Y.Z-<arch>-unknown-linux-musl.tar.gz` (binary, licence, README, shell completions)
   - `glori_X.Y.Z-1_<arch>.deb` and `glori-X.Y.Z-1.<arch>.rpm`
   - `SHA256SUMS` and a build provenance attestation for every file
4. A tag with a hyphen (`v1.0.0-rc.1`) is published as a pre-release; `glori self-update` ignores those.
5. Arch: bump `pkgver` in `packaging/arch/PKGBUILD`, set the real checksum with `updpkgsums`, and push it to the AUR.

`glori self-update` and `install.sh` both rely on the asset names and on `SHA256SUMS`; keep them stable.

## How people get Glori

| Way | Updates through |
| --- | --- |
| `curl -fsSL https://raw.githubusercontent.com/pozii/glori/main/install.sh \| sh` | `glori self-update` |
| `.deb` / `.rpm` from the release page | the package manager, or a manual download |
| `glori install <release tarball>` | `glori self-update` |
| Arch `PKGBUILD` | the AUR helper |

`glori self-update` refuses to touch a binary under `/usr`, `/bin`, `/sbin` or `/opt`; those belong to the package manager.

# Releasing

## Versioning

Glori uses calendar versioning: `YYYY.MM.DD`, with a counter for extra releases on the same day.

| Release | Version | Tag |
| --- | --- | --- |
| first of the day | `2026.11.17` | `v2026.11.17` |
| second of the day | `2026.11.17.1` | `v2026.11.17.1` |
| third of the day | `2026.11.17.2` | `v2026.11.17.2` |
| release candidate | `2026.11.17-rc.1` | `v2026.11.17-rc.1` |

- Month and day are zero padded in tags (`2026.01.05`, not `2026.1.5`). The counter is not and never starts at `.0`.
- The tag is the only source of truth. `Cargo.toml` stays at `0.0.0` because Cargo only accepts three-part versions; the release workflow injects the tag into the binary through `GLORI_VERSION`. A local build reports `0.0.0`.
- Ordering is numeric per part, so `2026.11.17 < 2026.11.17.1 < 2026.11.17.10 < 2026.11.18`. A release candidate sorts before its final release, and `glori self-update` ignores pre-releases.
- Use the date the release is cut, in UTC. Never reuse or move a published tag; publish the next counter instead.
- deb and rpm packages use the same version, with `~` in place of `-` for release candidates so they sort before the final release.

## Cutting a release

1. Make sure `main` is green.
2. Tag and push: `git tag v2026.11.17 && git push origin v2026.11.17`.
3. The workflow rejects tags that are not calendar versions, tests, builds static musl binaries for x86_64 and aarch64, and publishes a GitHub release with:
   - `glori-<version>-<arch>-unknown-linux-musl.tar.gz` (binary, licence, README, shell completions)
   - `glori_<version>_<arch>.deb` and `glori-<version>-1.<arch>.rpm`
   - `SHA256SUMS` and a build provenance attestation for every file
4. Arch: bump `pkgver` in `packaging/arch/PKGBUILD`, set the real checksum with `updpkgsums`, and push it to the AUR.

`glori self-update` and `install.sh` rely on the asset names and on `SHA256SUMS`; keep them stable.

## How people get Glori

| Way | Updates through |
| --- | --- |
| `curl -fsSL https://raw.githubusercontent.com/pozii/glori/main/install.sh \| sh` | `glori self-update` |
| `.deb` / `.rpm` from the release page | the package manager, or a manual download |
| `glori install <release tarball>` | `glori self-update` |
| Arch `PKGBUILD` | the AUR helper |

`glori self-update` refuses to touch a binary under `/usr`, `/bin`, `/sbin` or `/opt`; those belong to the package manager.

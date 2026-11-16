#!/bin/sh
set -eu

repo="pozii/glori"
base="${GLORI_BASE_URL:-https://github.com/$repo/releases}"
dest="${GLORI_INSTALL_DIR:-$HOME/.local/bin}"

fail() {
    echo "error: $*" >&2
    exit 1
}

need() {
    command -v "$1" >/dev/null 2>&1 || fail "$1 is required"
}

need curl
need tar
need sha256sum

case "$(uname -m)" in
    x86_64 | amd64) arch=x86_64 ;;
    aarch64 | arm64) arch=aarch64 ;;
    *) fail "unsupported architecture: $(uname -m)" ;;
esac

[ "$(uname -s)" = Linux ] || fail "Glori only supports Linux"

version="${GLORI_VERSION:-}"
if [ -z "$version" ]; then
    final=$(curl -fsSL -o /dev/null -w '%{url_effective}' "$base/latest") || fail "cannot find the latest release"
    version="${final##*/}"
fi
version="${version#v}"

name="glori-$version-$arch-unknown-linux-musl"
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

echo "Downloading Glori $version for $arch"
curl -fSL --progress-bar --proto '=https,http' -o "$work/$name.tar.gz" "$base/download/v$version/$name.tar.gz" || fail "download failed"
curl -fsSL --proto '=https,http' -o "$work/SHA256SUMS" "$base/download/v$version/SHA256SUMS" || fail "cannot download SHA256SUMS"

expected=$(grep "  $name.tar.gz\$" "$work/SHA256SUMS" | cut -d' ' -f1)
[ -n "$expected" ] || fail "no checksum listed for $name.tar.gz"
actual=$(sha256sum "$work/$name.tar.gz" | cut -d' ' -f1)
[ "$expected" = "$actual" ] || fail "checksum mismatch, refusing to install"

tar -xzf "$work/$name.tar.gz" -C "$work"
mkdir -p "$dest"
install -m 755 "$work/$name/glori" "$dest/glori.new"
mv -f "$dest/glori.new" "$dest/glori"

echo "Installed $("$dest/glori" --version) to $dest/glori"
case ":$PATH:" in
    *":$dest:"*) ;;
    *) echo "Add $dest to your PATH to use it." ;;
esac

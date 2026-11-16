#!/bin/sh
set -eu
target=${1:?usage: tarball.sh <rust target> <version>}
version=${2:?usage: tarball.sh <rust target> <version>}
arch=${target%%-*}
name="glori-$version-$arch-unknown-linux-musl"
stage="dist/$name"
rm -rf "$stage"
mkdir -p "$stage/complete"
cp "target/$target/release/glori" "$stage/glori"
cp LICENSE README.md "$stage/"
cp dist/completions/glori.bash dist/completions/glori.fish dist/completions/_glori "$stage/complete/"
tar -C dist -czf "dist/$name.tar.gz" "$name"
echo "dist/$name.tar.gz"

#!/bin/sh
set -eu
binary=${1:?usage: completions.sh <glori binary>}
out=dist/completions
mkdir -p "$out"
"$binary" completions bash > "$out/glori.bash"
"$binary" completions zsh > "$out/_glori"
"$binary" completions fish > "$out/glori.fish"

#!/usr/bin/env bash
# Downloads the Dovecot Pigeonhole sources (LGPL-2.1) into target/pigeonhole so
# their test scripts can be used as parser test input. Nothing is vendored.
#
#   scripts/fetch-pigeonhole.sh
#   PIGEONHOLE_DIR=target/pigeonhole cargo test -p sieve-core --test pigeonhole -- --ignored
set -euo pipefail

dest="$(cd "$(dirname "$0")/.." && pwd)/target/pigeonhole"
ref="${PIGEONHOLE_REF:-main}"

if [ -d "$dest/.git" ]; then
  git -C "$dest" fetch --quiet --depth 1 origin "$ref"
  git -C "$dest" checkout --quiet FETCH_HEAD
else
  mkdir -p "$(dirname "$dest")"
  git clone --quiet --depth 1 --branch "$ref" https://github.com/dovecot/pigeonhole.git "$dest"
fi
echo "Pigeonhole $(git -C "$dest" rev-parse --short HEAD) in $dest"

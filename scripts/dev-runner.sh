#!/bin/sh
# Cargo runner on macOS (see .cargo/config.toml). Signs the app binary with
# a stable identity before it starts, so the keychain recognizes it across
# rebuilds: an unsigned build looks like a new app to macOS each time and
# asks for the login keychain password on every start.
#
# Identity: $SIEVE_GUI_SIGNING_IDENTITY, else the first "Apple Development"
# identity in the keychain. Without one, the binary runs unsigned as before.
# Other binaries (tests, examples) run unchanged.
set -eu
bin=$1
if [ "$(basename "$bin")" = "sieve-gui" ]; then
  identity=${SIEVE_GUI_SIGNING_IDENTITY:-$(security find-identity -v -p codesigning 2>/dev/null |
    sed -n 's/.*"\(Apple Development: [^"]*\)".*/\1/p' | head -n 1)}
  if [ -n "$identity" ]; then
    codesign --force --sign "$identity" --timestamp=none "$bin" 2>/dev/null ||
      echo "dev-runner: signing with \"$identity\" failed, running unsigned" >&2
  fi
fi
exec "$@"

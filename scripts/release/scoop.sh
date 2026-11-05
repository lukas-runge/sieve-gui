#!/bin/sh
# Prints the Scoop manifest for a release (see .github/workflows/publish.yml).
#   scoop.sh <version> <sha256 of the x64 zip> <sha256 of the arm64 zip>
set -eu
version=$1 sha_x64=$2 sha_arm64=$3
base="https://github.com/lukas-runge/sieve-gui/releases/download"
cat <<EOF
{
  "version": "$version",
  "description": "Editor for server-side Sieve mail filters",
  "homepage": "https://github.com/lukas-runge/sieve-gui",
  "license": "MIT",
  "notes": "sieve-gui needs WebView2, which Windows 10 and 11 include.",
  "architecture": {
    "64bit": {
      "url": "$base/v$version/sieve-gui_${version}_x64_portable.zip",
      "hash": "$sha_x64"
    },
    "arm64": {
      "url": "$base/v$version/sieve-gui_${version}_arm64_portable.zip",
      "hash": "$sha_arm64"
    }
  },
  "bin": "sieve-gui.exe",
  "shortcuts": [["sieve-gui.exe", "sieve-gui"]],
  "checkver": "github",
  "autoupdate": {
    "architecture": {
      "64bit": { "url": "$base/v\$version/sieve-gui_\$version_x64_portable.zip" },
      "arm64": { "url": "$base/v\$version/sieve-gui_\$version_arm64_portable.zip" }
    }
  }
}
EOF

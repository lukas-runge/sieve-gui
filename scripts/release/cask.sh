#!/bin/sh
# Prints the Homebrew cask for a release (see .github/workflows/publish.yml).
#   cask.sh <version> <sha256 of the aarch64 dmg> <sha256 of the x64 dmg>
set -eu
version=$1 sha_arm=$2 sha_intel=$3
cat <<EOF
cask "sieve-gui" do
  arch arm: "aarch64", intel: "x64"

  version "$version"
  sha256 arm:   "$sha_arm",
         intel: "$sha_intel"

  url "https://github.com/lukas-runge/sieve-gui/releases/download/v#{version}/sieve-gui_#{version}_#{arch}.dmg"
  name "sieve-gui"
  desc "Editor for server-side Sieve mail filters"
  homepage "https://github.com/lukas-runge/sieve-gui"

  livecheck do
    url :url
    strategy :github_latest
  end

  depends_on :macos

  app "sieve-gui.app"

  # Not notarized yet: without the quarantine attribute Gatekeeper lets the
  # (ad-hoc signed) app open.
  postflight_steps do
    run "/usr/bin/xattr", args: ["-dr", "com.apple.quarantine", "{{appdir}}/sieve-gui.app"]
  end

  zap trash: [
    "~/Library/Application Support/de.lukasrunge.sieve-gui",
    "~/Library/Caches/de.lukasrunge.sieve-gui",
    "~/Library/WebKit/de.lukasrunge.sieve-gui",
  ]
end
EOF

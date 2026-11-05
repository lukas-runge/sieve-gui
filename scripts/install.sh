#!/bin/sh
# Installs sieve-gui on Linux from the latest GitHub release, or updates it:
# running it again installs the newest version over the old one.
#
#   curl -fsSL https://raw.githubusercontent.com/lukas-runge/sieve-gui/main/scripts/install.sh | sh
#
# Picks the package for the system: .deb with apt (Debian, Ubuntu, Mint, …),
# .rpm with dnf or zypper (Fedora, openSUSE, …), otherwise the AppImage in
# ~/.local/bin with a menu entry. `sh -s -- --appimage` takes the AppImage
# everywhere (no root needed).
#
# The app runs it with `--update <pid>` (a copy bundled into the app): wait
# for the app to quit, install, start sieve-gui again, whatever happened. With
# no terminal around, root comes from pkexec's password dialog. The output
# goes to ~/.local/state/sieve-gui/update.log.

set -eu

REPO="lukas-runge/sieve-gui"
API="https://api.github.com/repos/$REPO/releases/latest"
ICON_URL="https://raw.githubusercontent.com/$REPO/main/src-tauri/icons/128x128@2x.png"
DATA="${XDG_DATA_HOME:-$HOME/.local/share}"
# Where the AppImage goes; the app passes the one it runs from.
APPIMAGE_TARGET="${SIEVE_GUI_APPIMAGE:-$HOME/.local/bin/sieve-gui.AppImage}"

say() { printf '%s\n' "$*"; }
fail() { printf 'sieve-gui install: %s\n' "$*" >&2; exit 1; }
have() { command -v "$1" >/dev/null 2>&1; }

force_appimage=false
update_pid=""
while [ $# -gt 0 ]; do
  case "$1" in
    --appimage) force_appimage=true ;;
    --update)
      [ $# -ge 2 ] || fail "--update needs the app's process id."
      update_pid=$2
      shift
      ;;
    *) fail "unknown option: $1" ;;
  esac
  shift
done

[ "$(uname -s)" = Linux ] || fail "this script is for Linux. On macOS use Homebrew, on Windows winget (see the README)."
have curl || fail "curl is needed."

# The release names each package's architecture its own way.
case "$(uname -m)" in
  x86_64 | amd64) deb_arch=amd64 rpm_arch=x86_64 appimage_arch=amd64 ;;
  aarch64 | arm64) deb_arch=arm64 rpm_arch=aarch64 appimage_arch=aarch64 ;;
  *) fail "no release for $(uname -m)." ;;
esac

# How to become root: not at all, sudo in a terminal, pkexec from the app.
if [ "$(id -u)" -eq 0 ]; then
  root=""
elif [ -n "$update_pid" ] && have pkexec; then
  root="pkexec"
elif [ -z "$update_pid" ] && have sudo; then
  root="sudo"
else
  root="none"
fi

if $force_appimage; then
  format=appimage
elif have apt-get && have dpkg; then
  format=deb
elif have dnf || have zypper; then
  format=rpm
else
  format=appimage
fi
if [ "$format" != appimage ] && [ "$root" = none ]; then
  [ -z "$update_pid" ] || fail "updating needs pkexec to ask for the admin password."
  say "No sudo available: installing the AppImage for this user instead."
  format=appimage
fi

install_latest() {
  case "$format" in
    deb) pattern="_${deb_arch}\.deb$" ;;
    rpm) pattern="\.${rpm_arch}\.rpm$" ;;
    appimage) pattern="_${appimage_arch}\.AppImage$" ;;
  esac

  release=$(curl -fsSL -H "Accept: application/vnd.github+json" "$API") ||
    fail "couldn't get the latest release from GitHub ($API)."
  version=$(printf '%s\n' "$release" | sed -n 's/.*"tag_name": *"\([^"]*\)".*/\1/p' | head -n 1)
  url=$(printf '%s\n' "$release" |
    sed -n 's/.*"browser_download_url": *"\([^"]*\)".*/\1/p' |
    grep -E "$pattern" | head -n 1) || true
  [ -n "$url" ] || fail "the release ${version:-?} has no $format package for $(uname -m)."

  tmp=$(mktemp -d)
  trap 'rm -rf "$tmp"' EXIT
  file="$tmp/${url##*/}"

  say "Downloading sieve-gui $version ($format)…"
  curl -fL --progress-bar -o "$file" "$url" || fail "download failed: $url"

  case "$format" in
    deb)
      # apt resolves the dependencies (WebKitGTK and friends). Its sandbox
      # user has to be able to read the download.
      chmod 0755 "$tmp"
      chmod 0644 "$file"
      $root apt-get install -y "$file"
      ;;
    rpm)
      if have dnf; then
        $root dnf install -y "$file"
      else
        $root zypper --non-interactive install --allow-unsigned-rpm "$file"
      fi
      ;;
    appimage)
      bin=$(dirname "$APPIMAGE_TARGET")
      apps="$DATA/applications"
      icons="$DATA/icons/hicolor/256x256/apps"
      mkdir -p "$bin" "$apps" "$icons"
      # Next to the target, then renamed: the running file is never half written.
      install -m 0755 "$file" "$APPIMAGE_TARGET.new"
      mv -f "$APPIMAGE_TARGET.new" "$APPIMAGE_TARGET"
      curl -fsSL -o "$icons/sieve-gui.png" "$ICON_URL" || say "(Couldn't fetch the icon; the menu entry shows a generic one.)"
      cat >"$apps/sieve-gui.desktop" <<EOF
[Desktop Entry]
Type=Application
Name=sieve-gui
Comment=Edit server-side Sieve mail filters
Exec="$APPIMAGE_TARGET"
Icon=sieve-gui
Categories=Network;Email;
Terminal=false
EOF
      if have update-desktop-database; then update-desktop-database "$apps" >/dev/null 2>&1 || true; fi
      if [ -z "$update_pid" ]; then
        case ":$PATH:" in
          *":$bin:"*) ;;
          *) say "Note: $bin isn't on your PATH; start sieve-gui from the app menu or with $APPIMAGE_TARGET." ;;
        esac
        say "AppImages need FUSE 2 (libfuse2 / fuse2). If it doesn't start, install that package."
      fi
      ;;
  esac

  say "sieve-gui $version is installed."
}

if [ -z "$update_pid" ]; then
  install_latest
  say "Run this script again to update."
  exit 0
fi

# ── Update from the app ──────────────────────────────────────────────────────

log_dir="${XDG_STATE_HOME:-$HOME/.local/state}/sieve-gui"
mkdir -p "$log_dir"
exec >"$log_dir/update.log" 2>&1
say "Update started $(date)"

while kill -0 "$update_pid" 2>/dev/null; do sleep 0.2; done

# In a subshell so a failure ends only the install, not the restart. Not in
# an `if`: that would switch off `set -e` inside it.
set +e
(set -e; install_latest)
status=$?
set -e

if [ "$status" -ne 0 ]; then
  say "Update failed ($status)."
  if have notify-send; then
    notify-send -a sieve-gui "sieve-gui" "The update didn't go through. Details: $log_dir/update.log" || true
  fi
fi

if [ "$format" = appimage ]; then
  app="$APPIMAGE_TARGET"
else
  app=$(command -v sieve-gui || echo /usr/bin/sieve-gui)
fi
say "Starting $app"
nohup "$app" >/dev/null 2>&1 &

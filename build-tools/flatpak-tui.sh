#!/usr/bin/env bash
# The terminal `chor` as a Flatpak bundle:
# target/flatpak/petrichor64-tui-<version>-<arch>.flatpak
#
#   build-tools/flatpak-tui.sh           builds the TUI `chor` (bin-tui) first
#   build-tools/flatpak-tui.sh <chor>    packs a binary that's already built
#
# Install and run:
#   flatpak install --user petrichor64-tui-*.flatpak
#   flatpak run com.makeavoy.petrichor64.tui <game dir>
#
# The binary is built outside Flatpak and dropped into the app, rather than
# compiled by flatpak-builder: that build is offline, and build.rs clones the
# editors. So this needs only `flatpak` and the runtime:
#   flatpak install --user flathub org.freedesktop.Platform//25.08
set -euo pipefail
cd "$(dirname "$0")/.."

ID=com.makeavoy.petrichor64.tui
RUNTIME="${FLATPAK_RUNTIME:-25.08}"
ARCH="$(uname -m)"
VERSION="$(sed -n 's/^version *= *"\(.*\)"/\1/p' Cargo.toml | head -1)"
BIN="${1:-}"
if [ -z "$BIN" ]; then
  # Its own target dir: the TUI and the windowed build share no features, and
  # one target dir would rebuild everything each time you switch.
  cargo build --release --no-default-features --features bin-tui --target-dir target/tui
  BIN=target/tui/release/chor
fi
[ -x "$BIN" ] || { echo "no binary at $BIN"; exit 1; }

OUT=target/flatpak
APP="$OUT/app"
REPO="$OUT/repo"
mkdir -p "$OUT"
[ -d "$APP" ] && rm -r "$APP"
[ -d "$REPO" ] && rm -r "$REPO"

# build-init insists its sdk is installed, and nothing is compiled in here, so
# it is given the runtime (a quarter the download) and the metadata is put
# right afterwards.
flatpak build-init "$APP" "$ID" org.freedesktop.Platform org.freedesktop.Platform "$RUNTIME"
sed -i 's|^sdk=org.freedesktop.Platform/|sdk=org.freedesktop.Sdk/|' "$APP/metadata"
install -Dm755 "$BIN" "$APP/files/bin/chor"
# host: games are folders anywhere on disk, and `new` writes beside you.
# network: `conn`. pulseaudio: the runtime routes ALSA (cpal) through it.
# device=all: gamepads. Without udev and /dev/input the gamepad subsystem
# fails to start and takes the app's Lua thread with it.
flatpak build-finish "$APP" --command=chor \
  --filesystem=host --share=network --socket=pulseaudio --device=all
flatpak build-export "$REPO" "$APP"
FILE="$OUT/petrichor64-tui-$VERSION-$ARCH.flatpak"
flatpak build-bundle "$REPO" "$FILE" "$ID" \
  --runtime-repo=https://flathub.org/repo/flathub.flatpakrepo
ls -la "$FILE"

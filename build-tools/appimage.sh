#!/usr/bin/env bash
# The studio as an AppImage: target/appimage/Petrichor64-<version>-x86_64.AppImage
#
#   build-tools/appimage.sh            builds `chor` (default features) first
#   build-tools/appimage.sh <chor>     packs a binary that's already built
#
# Only `chor` goes in. What it links (libc, ALSA, udev, xcb) and what wgpu and
# winit load at run time (Vulkan/GL, Wayland, X11, xkbcommon) come from the
# system, as they do for a plain binary: bundling a second ALSA or graphics
# stack breaks more machines than it fixes. Build on an old distro for an old
# glibc (CI uses ubuntu-22.04).
#
# appimagetool is taken from $APPIMAGETOOL or PATH, else downloaded to target/.
set -euo pipefail
cd "$(dirname "$0")/.."

ARCH="$(uname -m)"
VERSION="$(sed -n 's/^version *= *"\(.*\)"/\1/p' Cargo.toml | head -1)"
BIN="${1:-}"
if [ -z "$BIN" ]; then
  cargo build --release
  BIN=target/release/chor
fi
[ -x "$BIN" ] || { echo "no binary at $BIN"; exit 1; }

OUT=target/appimage
DIR="$OUT/Petrichor64.AppDir"
mkdir -p "$OUT"
[ -d "$DIR" ] && rm -r "$DIR"
mkdir -p "$DIR/usr/bin"
install -m755 "$BIN" "$DIR/usr/bin/chor"
install -m755 build-tools/linux/AppRun "$DIR/AppRun"
install -m644 build-tools/linux/petrichor64.desktop "$DIR/petrichor64.desktop"
install -m644 petrichor-icon.png "$DIR/petrichor64.png"
ln -s petrichor64.png "$DIR/.DirIcon"

TOOL="${APPIMAGETOOL:-$(command -v appimagetool || true)}"
if [ -z "$TOOL" ]; then
  TOOL="$PWD/target/appimagetool-$ARCH.AppImage"
  if [ ! -x "$TOOL" ]; then
    echo "fetching appimagetool"
    curl -fsSL -o "$TOOL" \
      "https://github.com/AppImage/appimagetool/releases/download/continuous/appimagetool-$ARCH.AppImage"
    chmod +x "$TOOL"
  fi
fi

FILE="$OUT/Petrichor64-$VERSION-$ARCH.AppImage"
# --appimage-extract-and-run: appimagetool is an AppImage itself, and a CI
# runner or container has no FUSE to mount it with.
ARCH="$ARCH" "$TOOL" --appimage-extract-and-run "$DIR" "$FILE"
ls -la "$FILE"

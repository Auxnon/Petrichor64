# Petrichor64 build recipes. `just` (https://github.com/casey/just).
# Run `just` with no args to list everything.
#
# Feature map (see Cargo.toml):
#   default        = silt + headed   (Lua VM + wgpu window)
#   audio          = cpal synth (native only, opt-in)
#   render-tui     = software renderer for terminals (no GPU/window)
#   wasm           = browser build (no shared memory; CDN-friendly)
#   wasm-ultra     = wasm + SharedArrayBuffer (needs COOP/COEP headers)
#   include_auto   = bake auto.game.png into the binary (implies studio)
#   online_capable = networking (used by the headless server build)

# List recipes (default when you run `just`).
default:
    @just --list

# ---------------------------------------------------------------------------
# Native — run
# ---------------------------------------------------------------------------

# Run the engine (debug). Optionally pass a game dir/.game.png: `just run sounder`.
run game='':
    cargo run -- {{game}}

# Run with audio enabled (needed to hear sound): `just play sounder`.
play game='':
    cargo run --features audio -- {{game}}

# Run the terminal (software) renderer — no GPU/window.
tui game='':
    cargo run --no-default-features --features silt,render-tui -- {{game}}

# ---------------------------------------------------------------------------
# Native — build
# ---------------------------------------------------------------------------

# Release binary.
build:
    cargo build --release

# Release binary with audio.
build-audio:
    cargo build --release --features audio

# Release binary with a game baked in (reads ./auto.game.png).
build-cart:
    cargo build --release --features include_auto

# Headless server build (no GPU/window/Lua-default; networking only).
build-headless:
    cargo build --release --no-default-features --features online_capable

# macOS .app bundles (needs `cargo install cargo-bundle`).
bundle-mac-silicon:
    cargo bundle --release --target aarch64-apple-darwin

bundle-mac-intel:
    cargo bundle --profile release-nightly --target x86_64-apple-darwin

# macOS bundle with a game baked in.
bundle-mac-cart:
    cargo bundle --release --features include_auto --target aarch64-apple-darwin

# Full signed macOS dist (Silicon + Intel + headless), per build.sh.
dist-mac:
    ./build.sh

# Linux/SteamDeck AppImage (needs `cargo install cargo-appimage`).
appimage:
    cargo appimage

# ---------------------------------------------------------------------------
# Web (wasm) — via Trunk. Output name is fixed (filehash=false) so worker.js
# can import the bundle. `wasm` feature comes from web/index.html.
# ---------------------------------------------------------------------------

# Build the synth's own wasm module for the AudioWorklet, into web/.
#
# Separate from the Trunk build on purpose: the worklet needs a SMALL module (the
# engine's is ~3.4MB), and AudioWorkletGlobalScope has no fetch, so the main thread
# compiles this and hands it over by postMessage. `host-io` stays OFF here — the
# worklet is its own host, and none of the cpal/mic glue belongs in it.
# Trunk copies the two emitted files into dist (see web/index.html).
# Needs `cargo install wasm-pack`. Output goes to its own web/synth/ subdir
# because wasm-pack drops a catch-all .gitignore in its out-dir — pointed at
# web/ directly that would ignore index.html and worker.js too.
synth-wasm:
    wasm-pack build synth --release --target web --out-dir ../web/synth \
        -- --no-default-features --features worklet

# Dev server with live reload at http://localhost:8080.
# NOTE: Trunk.toml sets filehash=false (worker.js imports a fixed bundle name),
# so the browser caches Petrichor64_bg.wasm across rebuilds. After changing
# features/protocol, hard-reload with DevTools "Disable cache" on — otherwise
# the main thread and VM worker can run different vintages of the bundle (e.g.
# "unknown variant `Sound`" if one has audio and the other doesn't).
web: synth-wasm
    trunk serve

# Re-run to pick up game edits (the bundle is baked at build time); engine-source
# edits hot-reload on their own.
# Bundle a game as the embedded default, then start the dev server immediately.
web-serve game: synth-wasm
    cargo run -- pack {{game}} web/default.game.png
    trunk serve

# Release web build into web/dist.
web-build: synth-wasm
    trunk build --release

# SharedArrayBuffer build. REQUIRES cross-origin isolation: the page must be
# served with COOP `same-origin` + COEP `require-corp` headers, or
# SharedArrayBuffer is unavailable and it fails at runtime. `trunk serve` does
# not set these by default — put it behind a proxy or a header-capable server.
# wasm-ultra dev server (needs COOP/COEP cross-origin isolation).
web-ultra:
    trunk serve --features wasm-ultra

# Release wasm-ultra build.
web-ultra-build:
    trunk build --release --features wasm-ultra

# Packs the whole game (scripts + assets + sounds/*.ogg) into web/default.game.png,
# which the engine unzips at boot when no /game.game.png is deployed.
# Bake a game into the wasm build as the embedded default bundle, then build.
web-app game:
    cargo run --release -- pack {{game}} web/default.game.png
    trunk build --release

# The engine fetches /game.game.png at runtime, so games swap without a rebuild.
# Deploy build: release wasm + the packed game placed beside it in web/dist.
web-deploy game:
    trunk build --release
    cargo run --release -- pack {{game}} web/dist/game.game.png

# ---------------------------------------------------------------------------
# Tools
# ---------------------------------------------------------------------------

# Convert wav/mp3 under a dir to .ogg (for a game's sounds/ folder).
oggify dir='.':
    cargo run -p oggify -- {{dir}}

# Pack a game dir into <name>.game.png. `just pack sounder out.game.png`.
pack dir out='':
    cargo run --release -- pack {{dir}} {{out}}

# ---------------------------------------------------------------------------
# Quality
# ---------------------------------------------------------------------------

# Type-check the main feature combos + the wasm target.
check:
    cargo check
    cargo check --features audio
    cargo check --no-default-features --features silt,render-tui
    cargo check --target wasm32-unknown-unknown --features wasm

fmt:
    cargo fmt

test:
    cargo test

clean:
    cargo clean

# ---------------------------------------------------------------------------
# Android
# ---------------------------------------------------------------------------
# Nothing machine-specific is committed: the SDK comes from $ANDROID_HOME (falling
# back to the Android Studio default) and the NDK is whichever version is newest
# under it. Override either with `ANDROID_HOME=... ANDROID_NDK_HOME=... just android`.
#
# API 26 is a floor, not a preference: cpal's Android backend links `-laaudio`, and
# AAudio only exists from Android 8.0. Below that the link fails outright with
# "unable to find library -laaudio". If audio is ever made optional on Android, 24
# becomes possible again.
android_api := "26"

# Build the engine as an Android shared library (.so).
# `just android` for debug, `just android release` for a shippable one.
android profile="debug":
    #!/usr/bin/env bash
    set -euo pipefail
    SDK="${ANDROID_HOME:-$HOME/Library/Android/sdk}"
    [ -d "$SDK" ] || { echo "no Android SDK at $SDK (set ANDROID_HOME)"; exit 1; }
    NDK="${ANDROID_NDK_HOME:-$(ls -1d "$SDK"/ndk/* 2>/dev/null | sort -V | tail -1)}"
    [ -n "$NDK" ] && [ -d "$NDK" ] || { echo "no NDK under $SDK/ndk (install one in Android Studio)"; exit 1; }
    HOSTDIR="$(ls -1d "$NDK"/toolchains/llvm/prebuilt/* | head -1)"
    TC="$HOSTDIR/bin"
    TRIPLE=aarch64-linux-android
    echo "SDK $SDK"
    echo "NDK $NDK (API {{android_api}})"
    # The cc crate needs CC/CXX/AR per-target for any C/C++ dependency; rustc needs
    # the linker. The versioned clang wrapper bakes in the API level and sysroot,
    # which is why it's used rather than bare clang plus flags.
    export ANDROID_HOME="$SDK" ANDROID_NDK_HOME="$NDK"
    export CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER="$TC/${TRIPLE}{{android_api}}-clang"
    export CC_aarch64_linux_android="$TC/${TRIPLE}{{android_api}}-clang"
    export CXX_aarch64_linux_android="$TC/${TRIPLE}{{android_api}}-clang++"
    export AR_aarch64_linux_android="$TC/llvm-ar"
    FLAG=""; [ "{{profile}}" = "release" ] && FLAG="--release"
    # --lib only: the `Petrichor64` bin has no meaning on Android (the activity
    # loads the cdylib and calls android_main), and building it just wastes time.
    cargo build --target "$TRIPLE" --lib $FLAG
    OUT="target/$TRIPLE/{{profile}}/libpetrichor64.so"
    ls -la "$OUT"
    # The activity resolves these by name; if either is missing the app dies at
    # startup with no useful message, so check rather than assume.
    "$TC/llvm-nm" --defined-only --dynamic "$OUT" | grep -qE ' T android_main' \
      && echo "ok: android_main exported" || { echo "MISSING android_main"; exit 1; }
    "$TC/llvm-nm" --defined-only --dynamic "$OUT" | grep -qE ' T ANativeActivity_onCreate' \
      && echo "ok: ANativeActivity_onCreate exported" || { echo "MISSING ANativeActivity_onCreate"; exit 1; }

# Type-check for Android without needing the NDK (checking doesn't link, so this
# works with only `rustup target add aarch64-linux-android`). Kept out of `just
# check` so that recipe still works for anyone who hasn't added the target.
check-android:
    cargo check --target aarch64-linux-android --lib

# Add the other ABIs (32-bit phones, and the x86_64 emulator).
android-targets:
    rustup target add aarch64-linux-android armv7-linux-androideabi x86_64-linux-android

# Tail the engine's logcat output. Android has no stdout, so this is where
# `log::info!` and any panic actually surface.
android-log:
    #!/usr/bin/env bash
    SDK="${ANDROID_HOME:-$HOME/Library/Android/sdk}"
    exec "$SDK/platform-tools/adb" logcat -v color petrichor64:V RustStdoutStderr:V '*:S'

# Print the shell exports worth having on PATH (adb, sdkmanager, emulator).
# Eval it or paste it into your shell rc: `just android-env >> ~/.zshrc`
android-env:
    #!/usr/bin/env bash
    SDK="${ANDROID_HOME:-$HOME/Library/Android/sdk}"
    NDK="$(ls -1d "$SDK"/ndk/* 2>/dev/null | sort -V | tail -1)"
    echo "export ANDROID_HOME=\"$SDK\""
    echo "export ANDROID_NDK_HOME=\"$NDK\""
    echo "export PATH=\"\$PATH:$SDK/platform-tools:$SDK/emulator:$SDK/cmdline-tools/latest/bin\""

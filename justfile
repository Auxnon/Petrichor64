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

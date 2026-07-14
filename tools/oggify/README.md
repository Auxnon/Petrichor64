# oggify

Convert audio files to **OGG Vorbis** for Petrichor64's `sounds/` folder.

```sh
cargo run -p oggify -- <dir>     # dir defaults to "."
```

Walks `<dir>` recursively, converts every `.wav`/`.mp3` into a sibling `.ogg`,
then asks whether to delete the originals (**default: no** — a bare Enter keeps
them). Existing `.ogg` files are left alone.

## Why it's a separate crate

The engine loads sounds by **decoding** ogg with `lewton` (pure Rust, wasm-safe,
no C). *Encoding* to Vorbis has no mature pure-Rust option, so oggify uses
`vorbis_rs`, which vendors libvorbis/libogg **C** source and builds it with `cc`.
Keeping that here means the C dependency never touches the size-optimized engine
binary or the wasm build. oggify is native-only and never a dependency of the
engine (it's a workspace member you build explicitly).

## Deps
- `symphonia` (decode wav/mp3 → f32 PCM; pure Rust)
- `vorbis_rs` (encode → ogg; vendored C, built via `cc` — needs a C compiler at
  build time, but no system libraries to install)
- `walkdir`

## Possible future work
- More input formats (flac, aac) — add symphonia features.
- A quality/bitrate flag (currently uses the encoder default VBR).
- Fold a decode-only path into the engine's asset packer so `sounds/` gets
  bundled into `.game.png` (see PLAN.md → Sound System "Remaining / gaps").

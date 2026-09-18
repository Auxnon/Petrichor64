# Backend plan: Renderer trait + TUI backend

Handoff doc for whichever agent picks this up next. Read this before touching
`src/tui/`, `src/root.rs`, `src/ent_manager.rs`, or Cargo feature flags.

## Goal

Support low-power / GPU-less devices (Raspberry Pi Zero 2 W class and similar)
by giving petrichor64 a second, software-only rendering path that draws the
3D world into a terminal via ANSI block characters, alongside the existing
100%-wgpu path. Long term this implies a `Renderer` trait boundary; short
term (what's actually built) is a standalone `render-tui` feature that does
not touch the wgpu path at all.

## What exists right now (already committed)

- New Cargo feature `render-tui = ["crossterm"]`, fully independent of
  `headed`. `headed` is unchanged — still means "wgpu path," unchanged deps.
  `crossterm` is an optional dep under
  `[target.'cfg(not(target_arch = "wasm32"))'.dependencies]`.
- `src/tui/mod.rs`, `src/tui/renderer.rs`, `src/tui/input.rs` — new module,
  gated `all(feature = "render-tui", not(target_arch = "wasm32"))`.
  - `tui::start()` mirrors the existing headless `start()` in `lib.rs`
    (same `Core::new()`, same tick shape). Wired into `lib.rs`'s `pub fn
    start()` dispatcher, gated `all(not(feature = "headed"), feature =
    "render-tui")`.
  - `src/tui/renderer.rs` reads `ChunkModel.vert_data` / `.ind_data`
    directly for terrain (these fields turned out to already be
    unconditional / un-gated — only `.buffers` / `.cook()` are
    wgpu-specific) and downcasts `EntManager`'s headless
    `Vec<UserDataWrapper>` to `LuaEnt` for entity positions, drawing each
    as a `ModelManager::CUBE` marker (no per-model shape/rotation yet —
    that's the first real gap, see Next steps).
  - `src/tui/input.rs` builds a `ControlState` bitset from crossterm key
    events (its own KeyCode→index mapping, independent of `controls.rs`'s
    winit-typed one) and feeds it through the existing
    `BundleManager::call_loop`, so Lua's `key()` and existing camera/game
    scripts work unmodified.
  - `Cargo.toml` `compile_error!` guard prevents `headed` and `render-tui`
    being enabled together (`Core` can only hold one concrete renderer).
- `src/root.rs`'s headless `Core::new()` gates its background stdin-reader
  thread behind `#[cfg(not(feature = "render-tui"))]` — crossterm puts the
  terminal in raw mode and reads stdin itself; a second blocking
  line-reader thread on the same fd would race it for bytes.
- `Vertex` in `model.rs` gained `pub fn pos()/normal()/tex()` getters
  (fields were private) so the rasterizer can read packed vertex data
  cross-module. This and the stdin-thread gate above are the *only* two
  changes made to pre-existing shared code for the TUI path — everything
  else in `ent.rs`, `model.rs`'s existing logic, `tile.rs`,
  `ent_manager.rs`, `gfx.rs`, `gui.rs` is untouched by design (see
  "Why the scope shrank" below).
- **Real bug fix, unrelated to TUI but found while getting this to
  compile**: `EntRef::with_mut` in `ent_manager.rs` needed
  `where R: for<'a> silt_lua::value::ToLua<'a>` added to its signature —
  a newer `silt-lua` requires `R: ToLua<'b>` on `downcast_mut` but the
  method's own generic bound didn't declare it. Without this the build
  fails with `the trait bound silt_lua::Value<'_>: From<R> is not
  satisfied`. This is what "downcast in ent_manager" errors trace back to
  if they resurface after a `silt-lua` bump.

## Verified state (this session, on this machine)

```
cargo build                                              → 0 errors, 50 warnings
cargo build --no-default-features --features silt,render-tui → 0 errors, 108 warnings (last full check)
cargo check (default/headed)                             → 0 errors, 50 warnings (just re-verified)
```

User has run the `render-tui` build and confirmed it launches and shows a
bare TUI (terrain/entities render as block art; no crash). Camera/input
wiring has not been separately confirmed working end-to-end — worth
double-checking key-driven camera movement actually moves the view.

The 50 remaining warnings in the default build are **all cosmetic**:
non-local `macro_rules!` in `command.rs`, a no-op `drop(&reference)` at
`command.rs:1990`, and snake_case naming lints on intentionally-SCREAMING
or camelCase identifiers (`ModelManager::CUBE/PLANE/DICTIONARY`,
`World::COUNTER/INT_TEX_DICTIONARY/...`, `ent_manager.rs`'s
`targetId/childId/parentIndex/childIndex`). None of these are bugs.

## Uncommitted working-tree state — read before doing anything else

`git diff HEAD` currently shows ~20 modified files (`asset.rs`,
`command.rs`, `bundle.rs`, `controls.rs`, `ent.rs`, `ent_manager.rs`,
`error.rs`, `gui.rs`, `lib.rs`, `lua_define.rs`, `lua_ent.rs`, `lua_img.rs`,
`model.rs`, `parse.rs`, `pool.rs`, `ray.rs`, `render.rs`, `texture.rs`,
`tile.rs`, `world.rs`) that are **not part of the commit the user already
made** (`f7553fe "feat: tui work, more unused var correction"`). These are
mechanical warning-suppression edits from an earlier cleanup pass:
`some_call(...)` → `let _ = some_call(...)` on ignored `Result`s, and
unused function/closure params renamed to `_name`.

**The user explicitly said they'd rather remove unused params outright
than leave them prefixed with `_`, but hasn't decided, and wants the
warnings left alone for now.** Do not commit this diff and do not run any
further automated warning-fix pass (`cargo fix`, scripted sed-style
replacements) without asking first. If you want to clean this up, the
better fix per file is almost always to actually delete the dead
parameter/binding rather than underscore-prefix it — but confirm that's
what's wanted before doing a sweep.

## Hazard: do NOT run `cargo fix` under a non-default feature set

This bit us once already. Running
`cargo fix --lib -p Petrichor64 --no-default-features --features
silt,render-tui --allow-dirty` to clean up render-tui-only warnings deleted
imports and renamed params to `_`-prefixed forms **at their definition
sites** — which are shared with `#[cfg(feature = "headed")]`-gated code
elsewhere in the same functions. That silently broke the default/headed
build with 168 fresh compile errors, because `cargo fix` only sees the
currently-active feature config's view of "what's used." Full recovery
took a file-by-file diagnostic-driven pass restoring every import/mut
binding.

**Rule going forward: never run `cargo fix` scoped to a non-default
feature combination on this codebase without immediately rebuilding every
other feature combination afterward.** Prefer hand-fixing cross-cutting
import/mut/param warnings instead of trusting an automated pass when more
than one feature config touches the same file.

## Why the scope shrank from the original design

The original plan (see git history / prior planning doc if still present)
called for a proper `Renderer` trait with `Gfx` and a new `TuiRenderer`
both implementing it, splitting `ChunkModel`/`EntityUniforms`/`render_loop`
into backend-agnostic vs. wgpu-specific halves. Mid-implementation,
`grep -rc 'feature = "headed"' src/*.rs` turned up **286 gate sites across
16 files** (`model.rs` alone: 67) — `ent.rs`/`gfx.rs`/`controls.rs` are
whole modules only compiled under `headed`. Cleanly splitting that would
have meant editing a lot of code that currently works, for uncertain
payoff at v1. Re-scoped (with user sign-off) to a **leaner, fully
independent `render-tui` feature** that duplicates just enough logic
(input mapping, a standalone rasterizer, entity/terrain read paths) to
stand alone, rather than sharing a trait with the wgpu path.

The formal `Renderer` trait extraction described above is still the
long-term direction if a second GPU-capable low-power backend (e.g. a
wgpu-GL path for the Pi Zero 2 W) is wanted later — at that point sharing
an abstraction between `Gfx` and something else becomes worth the
refactor risk. Not needed for the TUI path itself.

## Next steps / open items

1. **Decide on the uncommitted warning-fix diff** (see above) — remove
   dead params vs. keep `_`-prefixed — then commit or discard it.
2. TUI entities currently all render as the same `CUBE` marker regardless
   of their actual model/rotation — first visible fidelity gap.
3. No depth-buffer/backface-cull confirmed correctness — original plan
   called for a half-block (`▀`) double-vertical-resolution trick; check
   whether `src/tui/renderer.rs` actually implements this or is doing
   something simpler.
4. Confirm camera movement via crossterm input actually drives
   `controls_evaluate`/Lua `key()` end-to-end in a live terminal, not just
   "it launches without crashing."
5. No automated test coverage for either render path — verification is
   manual run-and-look. If reworking the TUI renderer, re-run it against
   `test/basic` (or whichever bundle) in a real terminal before trusting
   changes.
6. Out of scope, deliberately deferred: console/GUI overlay in the TUI,
   CRT post-effect equivalent, wgpu-GL backend for Pi Zero 2 W, Miyoo Mini
   v2 (GLES1/2 via `glow`, unrelated to any of this).

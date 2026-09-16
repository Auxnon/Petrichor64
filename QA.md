# QA — native commands and their test coverage

Tracks the Lua-facing surface added in each session against what's actually
been checked, so the next agent (or the owner) knows what's proven and what's
still "compiled and ran without crashing" rather than "confirmed correct."

**What "live-verified" means here**: this sandbox has no display. "Live" means
a scratch script (usually in the scratchpad, deleted after) was actually run
against the built binary and its `cout()` output and process exit/log
inspected for errors/panics/wgpu validation failures. It does **not** mean
anyone looked at the screen. Anything visual (shadow shape/darkness, light
falloff, tint colour, CRT look) is explicitly called out as unverified below
— treat those numbers as first-pass, not tuned.

Add a section per feature as it lands. Keep entries short: command, unit
tests (file + test name), what the live check exercised, what's still open.

---

## Graphics chip / display monitor — `chip()`, `mon()`

Render-pipeline presets (`src/command.rs`, `src/root.rs::apply_chip`/
`apply_monitor`, `src/shaders/shader.wgsl`). See `guide/chip.md`, `guide/mon.md`.

- **Unit tests**: none directly (pure Rust preset tables, no interesting
  branch logic to unit test). Reset-on-load covered indirectly by
  `global::tests::app_attrs_do_not_leak_into_the_next_app`.
- **Live-verified**: `apps/rig` (the showroom app) run interactively-by-script
  cycling all 3 chips × 3 monitors — confirmed `chip()`/`mon()` calls don't
  error and `apps/rig`'s `mod()` cube-texture call works (caught and fixed a
  real bug here: the cube-form `mod()` call needs `{t = {...}}`, not a bare
  array — the guide's old prose example was wrong).
- **Unverified**: the actual visual look of R43/R30/Slot/Grille — wobble,
  dithering, blur, CRT curvature. Numbers are first-pass.

## Chip/monitor file header — `--! 0xNN`

`src/asset.rs::parse_chip_header`. See `guide/mon.md`.

- **Unit tests** (`src/asset.rs`, `mod chip_header_tests`):
  - `valid_header_is_parsed_and_consumed`
  - `no_header_leaves_the_buffer_untouched`
  - `malformed_header_falls_back_cleanly`
  - `zero_byte_maps_to_r00_and_lcd`
  - `out_of_range_nibbles_fall_back_to_zero`
  - `ordinary_comment_is_not_mistaken_for_a_header`
- **Live-verified**: a scratch script with `--! 0x21` as its first line,
  confirmed `chip()`/`mon()` read back `r30`/`slot` before `main()` ran, and a
  header-less script confirmed the `r00`/`lcd` default.

## Collision — `hit.all()`, `hit.cell(id)`, `hit.pair(a, b)`, `hit_shape`/`hit_size`/`hit_offset`

`src/collide.rs` (shape math), `src/ent_manager.rs` (broad phase, collider
derivation), `src/world.rs`/`src/tile.rs` (direct tile-read mirror),
`src/command.rs`/`src/lua_ent.rs` (Lua API). See `guide/hit.md`.

- **Unit tests** (`src/collide.rs`, `mod tests`):
  - `separated_boxes_do_not_hit`
  - `overlapping_boxes_push_along_least_overlap_axis`
  - `box_box_normal_points_away_from_b`
  - `cylinders_push_radially_and_ignore_far_z`
  - `cylinder_vs_box_corner_gives_true_contact_normal`
  - `cyl_box_normal_direction_is_consistent_regardless_of_argument_order`
  - `tile_box_matches_the_tile_to_world_convention`
  - Plus `src/tile.rs::chunk_key_tests::is_tile_in_matches_the_div_euclid_chunk_key_convention`
    (pins the direct-read mirror's chunk-key math against `Layer`'s own).
- **Live-verified**: a scratch script covering the full matrix —
  cylinder-vs-tile (`hit.cell`, confirmed tile coords/normal/depth by hand:
  `depth=16` for a concentric cylinder-in-tile case, matched hand computation
  exactly), cylinder-vs-cylinder (`hit.pair`, `depth≈6.4` matched hand
  computation), the batch query (`hit.all()`), a non-overlapping pair
  returning `nil`, and `hit_size` override actually shrinking a collider
  enough to end an overlap. Caught and fixed two real bugs in the process: a
  Lua array-table construction bug (elements came back `nil` despite `#t`
  being correct — fixed by using `table_from_array` instead of manual
  indexed `.set()`), and a unit-consistency bug (collider sizes weren't
  scaled by the engine's ×16 tile-to-world convention, so colliders were 16×
  too small relative to real distances).
- **Unverified**: nothing behavioral — the hand-checked math is real proof
  here, not just "didn't crash." Not tested: entities parented via `group()`
  interacting with collision (no special-casing was added; a child's
  collider ignores its parent's transform), and box-vs-box against a real
  model asset (only the tile case, itself a box, was exercised end-to-end;
  the box_box math itself is unit-tested but not through the full
  Lua→collider-derivation path for a non-tile model entity).

## Lighting — `lum{}` (replaces `lamp{}`)

`src/global.rs`, `src/command.rs`, `src/shaders/shader.wgsl::compute_shade`.
See `guide/lum.md`. `apps/synth` updated (`lamp(` → `lum(`) since it's a real
existing caller.

- **Unit tests**: none — this is shape-branched shading math, verified via
  runtime/shader-compile checks instead (see below), not isolable as pure
  Rust the way `collide.rs` is.
- **Live-verified**: three separate scratch runs (sun/default, `shape="cone"`,
  `shape="sphere"`), each run to completion (30 frames) with no shader
  compile errors, no wgpu validation errors, no panics. `cargo check` cannot
  catch WGSL/pipeline errors at all — this runtime check is the only signal
  available in this sandbox.
- **Unverified**: the actual look of falloff/angle/hemisphere-ambient.

## Shadow map — `shdw()`

`src/gfx.rs` (shadow pipeline/bind group/128×128 depth target), `src/render.rs`
(shadow pass, light-space matrix), `src/shaders/shader.wgsl` (`shadow_vs_main`,
shadow sampling in `fs_main`). See `guide/shdw.md`.

- **Unit tests**: none — same reasoning as `lum{}` above (pipeline/shader
  wiring, not isolable pure-Rust logic).
- **Live-verified**: `shdw(true)` with `sun`, with `cone`, and with `sphere`
  (must be a no-op — sphere can't shadow, confirmed the pass is skipped
  without erroring) — each run 30+ frames with no validation errors. Also
  verified `shdw()` combined with `gour()` and a `tint` in the same frame
  (see below) to catch any bind-group/uniform-layout interaction bugs between
  features landed in the same session.
- **Unverified**: shadow position/size/darkness/softness/bias — all
  first-pass constants (128px map, 400-unit ortho radius, 0.002 depth bias).

## Shading mode + vertex tint — `gour()`, `entity.tint`

`src/lua_ent.rs` (`tint` field), `src/ent.rs` (`get_uniforms_with_mat` reads
`lua.tint` instead of the old always-`GREEN` `Ent.color`), `src/shaders/shader.wgsl`
(`compute_shade` shared by `vs_main`/`fs_main`, tint multiply in `fs_main`).
See `guide/gour.md`, `guide/entity.md`.

- **Unit tests**: none — same reasoning as `lum{}`/`shdw()`.
- **Live-verified**: `gour(true)` alone; `tint` alone; and all three new
  systems together (`shdw(true)` + `gour(true)` + `tint` + a real `lum` sun +
  a solid tile), each run 30+ frames clean. Confirmed the pre-existing
  `GREEN` default landmine is gone by checking a plain `apps/model` run
  (no `lum`/`tint` calls at all) still renders with no shader errors —
  zero-behavior-change for apps that never touch any of this.
- **Unverified**: tint colour correctness, and whether Gouraud's per-vertex
  wobble actually looks meaningfully different from smooth on the low-poly
  test assets used (`dot`/`cube` — not enough triangle density to tell by
  reasoning alone; needs eyes on a real mesh).

## Entity-owned tile grids — `ent:tile`/`dtile`/`istile`/`gtile`/`ftile`, `hit.grid(id)`

`src/tile_grid.rs` (the `TileGrid`/`GridChunk`/`GridCell` data, self-owned on
`LuaEnt` — headless-safe), `src/lua_ent.rs` (the colon methods), `src/collide.rs`
(`test_grid`, the 3-tier cascade + local-space-transform trick),
`src/ent_manager.rs` (`grid_snapshot`/`hit_grid`, and the headed-only
`entity_grid_models` render-mesh cache + `check_entity_grids`), `src/tile.rs`
(`ChunkModel::build_grid_chunk`/`update_transform`, `_add_named_tile_model`),
`src/render.rs` (draw loops for grid-owning entities, shadow + main pass). See
`guide/entity.md`, `guide/hit.md`, `guide/grid.md`.

- **Unit tests**:
  - `src/tile_grid.rs`, `mod tests` (7 pre-existing + 1 new this pass):
    `unpack_local_index_is_the_inverse_of_local_index` (round-trips the packed
    cell index the mesher/cascade both rely on).
  - `src/collide.rs`, `mod tests` (3 new): `test_grid_finds_a_hit_in_an_axis_aligned_grid`,
    `test_grid_rejects_when_the_entity_aabb_misses`, and
    `test_grid_rotates_the_normal_back_to_world_space` — the last one hand-computes
    a 90°-rotated, translated grid and checks the returned world-space normal
    and depth exactly (not just "a hit happened"), proving the local-space-
    transform trick's rotate-the-normal-back step is correct, not just
    plausible.
- **Live-verified**: a scratch app (`make()` two entities, `owner:tile('cube',
  0,0,0,0)`, `owner.rz = tau/4`, `owner.x = 100/16` — the exact scenario the
  unit test hand-computes) run to completion (`quit(0)`, clean exit) for 9
  frames, calling `hit.grid(probe.id)` every frame. Output matched the unit
  test's hand-computed numbers exactly: `tile={0,0,0}`, `normal≈{0,-1,0}`,
  `depth≈4.0}`, every frame, no drift. No panics, no wgpu validation errors in
  the log.
- **Bug found and fixed by this live run**: `ChunkModel::create_buffer`'s
  instance buffer was created with `wgpu::BufferUsages::VERTEX` only — fine
  for the world's own chunks, which only ever write it once at `new()`, but
  an entity-grid chunk's owner can move every frame, so `update_transform`
  calls `queue.write_buffer` on it each frame. Writing to a buffer without
  `COPY_DST` doesn't error, it **hangs the GPU queue** — no panic, no log
  line, the process just stops making progress. Root-caused by bisecting with
  `eprintln!` tracepoints through `check_entity_grids` until the hang
  localized to one `queue.write_buffer` call, then confirmed against the
  Vertex-only usage flags. This class of bug — a silent driver-level hang,
  not a validation error — is exactly why this session's methodology insists
  on live-running the actual binary rather than trusting a clean
  `cargo check`/`cargo test`.
  Fixed via a new `ChunkModel::new(..., updatable: bool)` parameter rather
  than blanket-adding `COPY_DST` everywhere: the world's own chunks never
  move, so they keep the plain `VERTEX`-only buffer (marginally cheaper, and
  "can this ever be written again" stays statically visible at the call
  site) — only entity-grid chunks (`ent_manager.rs`'s rebuild, `updatable:
  true`) get `COPY_DST`. `world.rs`'s call site passes `false` explicitly.
- **Unverified**: nothing behavioral about the cascade itself (the hand-
  checked live numbers are real proof, matching the unit test bit-for-bit).
  Not exercised live: a `Cyl` query against a rotated grid (only `Box` was
  live-tested; `Cyl` is unit-tested indirectly through `collide::test`'s
  existing coverage, not through `test_grid` specifically), multiple chunks
  per grid, `dtile`/`clear` actually dropping a cached render mesh (the
  reconciliation logic runs but wasn't watched happen), and the visual
  correctness of a moving/rotating grid's mesh (no display in this pass
  either — same "ran clean, not eyeballed" caveat as `lum`/`shdw`/`gour`).

## `render-tui` backend — entity mesh/rotation, and a compile-baseline fix

`src/tui/renderer.rs` (`TuiRenderer::render_frame`'s entity loop), `src/gui.rs`
(`Gui::render`), `src/root.rs` (`Core::resize`). Software rasterizer for the
terminal backend (`--no-default-features --features silt,render-tui`) —
world terrain already had real per-triangle rendering; entities were drawn as
uniform, unrotated cube markers regardless of `asset`/rotation. This pass
gives entities their real model and orientation.

- **Prerequisite fix**: `render-tui` didn't compile at all — 11 errors, all
  from two functions using headed-only types (`wgpu::Queue`, `winit::dpi::
  PhysicalSize`, `Core.gfx`) and headed-only `Gui` methods/fields
  (`apply_console_out_text`, `process_notifications`, the five `*_layer`
  fields) without being `#[cfg(feature = "headed")]`-gated themselves. Both
  functions' only callers (`src/render.rs:108` and the winit `ApplicationHandler`
  impl in `lib.rs`) are already headed-only, so gating the two functions
  (`Gui::render`, `Core::resize`) is a correct, minimal fix — not a
  workaround. `cargo check --no-default-features --features silt,render-tui`
  now compiles clean (0 errors); `cargo check --features audio` (the default
  headed build) still compiles clean too, confirming nothing regressed there.
- **What changed**: the entity loop in `render_frame` now resolves each
  `LuaEnt`'s `asset` via `ModelManager::get_model_or_not` (falling back to
  the cube model for a sprite/billboard asset with no 3D shape, mirroring
  `ent_manager.rs::check_bundle_ents`'s same fallback), and composes the
  model matrix as translation * scale * rotation — `glam::Quat::from_euler`
  from `rot_x/rot_y/rot_z` — mirroring `Ent::build_meta` in `src/ent.rs`
  exactly (minus the `offset` term, not carried over: no test case needed it
  and it'd add a term this backend's docs don't currently promise).
- **Live-verified**: temporarily swapped `test/basic/scripts/basic.lua`
  (restored byte-for-byte after, confirmed via `git diff --stat test/basic`
  showing no diff) for a scratch script spawning two cube entities — one
  plain, one rotated on `rz`/`rx` — with a `cam{}` call, then ran
  `cargo run --no-default-features --features silt,render-tui --` in this
  environment's real terminal (redirected to a log + `timeout`, since the
  TUI loop only exits on an Esc keypress — `quit()` from Lua does not stop
  it; a real gap, but out of scope for this pass, noted below). Confirmed via
  the raw ANSI output: non-sky-color pixels present in a shape consistent
  with two cube-ish objects at the expected screen position for the test
  camera, and no panics. Caught and fixed one verification mistake of my
  own along the way, not a code bug: grepping for the triangle's literal
  base color found nothing, because `rasterize_tri`'s lighting term
  (`normal.dot(light_dir).max(0.15)`) scales it down — the color codes that
  actually appear (`33;21;9`, i.e. `220,140,60` × the 0.15 floor) confirmed
  real triangles were drawn once I grepped for the right thing.
- **Unverified / left open**:
  - Every triangle observed hit the `lit` floor of exactly `0.15` — the
    surface normal is apparently never facing enough toward `light_dir =
    (0.4,0.4,0.82)` to read above the clamp for either entity/orientation
    tried. Not investigated further (pre-existing shading code, untouched by
    this pass, and not blocking — shapes are still visible and distinct from
    the sky) but likely worth a look if the terminal output ever looks
    unlit/flat in practice.
  - `quit()` from Lua does not stop the TUI loop (`src/tui/mod.rs`'s loop
    only breaks on `TuiInput::poll()`, i.e. Esc) — a real gap in this
    backend, found but not fixed here since it's outside this pass's scope
    (entity rendering + the compile baseline).
  - `src/tui/mod.rs::start()` hardcodes `"test/basic"` and ignores whatever
    game path is passed on the command line — also found, also out of scope
    here, and the reason live verification above edited `test/basic` in
    place rather than pointing the binary at a scratch app.
  - Rotation was only eyeballed via raw pixel/color presence, not a
    pixel-exact comparison against hand-computed geometry (unlike the
    entity-grid collision work above) — reasonable for a terminal
    half-block renderer where "does it look different" is the actual bar,
    but flagged for the same reason every other visual claim in this doc is.

### Follow-up: `start()`'s hardcoded game path, and `apps/spin`

Fixed the "`src/tui/mod.rs::start()` ignores the CLI game path" gap noted
above — it now uses the same `env::args().nth(1)`-then-`check_for_auto()`
precedence `lib.rs`'s own `start()` paths use, instead of hardcoding
`"test/basic"`. `quit()` still doesn't stop the TUI loop (Esc only) — still
open, still out of scope here.

Added `apps/spin` — a minimal example app (a single cube, `rx`/`ry`/`rz`
incremented at three different rates each `loop()`) for eyeballing this
backend's geometry: `cargo run --no-default-features --features silt,render-tui
-- apps/spin` (or `just tui apps/spin`).

- **Live-verified**: ran the built binary against `apps/spin` for 3s
  (`timeout`, output redirected to a log — Esc-only exit means a real TTY
  session is the normal way to stop it). Confirmed in the log: `~loaded into
  game apps/spin` (proving the CLI-arg fix actually took effect, not a stale
  `test/basic` load), 166 distinct frames captured, and splitting the log on
  the `\x1b[H` cursor-home marker + hashing the first/middle/last frame's raw
  bytes showed all three differ — the rendered image is genuinely changing
  frame to frame, not static. Non-sky color codes present at multiple
  brightness levels (`33;21;9` through `181;115;49` — the cube's base color
  at different `lit` values), consistent with several distinct faces of a
  rotating cube being visible over the capture window. No panics.
- **Unverified**: same caveat as above — pixel presence/change over time is
  evidence of "something is rotating," not a pixel-exact check that the
  rotation matches the exact `rx`/`ry`/`rz` values commanded.

---

### `apps/art` — 4-colour interlacing-dither painter, and two TUI backend bugs found along the way

Added `apps/art`, cloning `apps/paint`'s 3D-canvas-plus-pointer-unprojection
design but reworked per spec: a fixed 4-colour palette (black/cyan/magenta/
yellow), two brush kinds (`full` overwrites every pixel in the footprint,
`dither` overwrites a checkerboard half of it), and a UI moved entirely into
3D space — a floating canvas plane pushed back to `CANVAS_DIST` and six
floating cube "buttons" (4 colours + 2 brush kinds) hanging closer to the
camera at `BUTTON_DIST`, hit-tested via the same ray/plane-intersection trick
FRESCO uses for the canvas, generalized to `plane_hit(m, dist)` and tried
nearest-first (buttons before canvas, since they're meant to occlude it).

The interlacing trick: a single global `dither_flip` (0/1) flips every time a
*new* stroke starts on the canvas (not on button taps). `dither` brush pixels
are only painted where `(x+y) % 2 == dither_flip`; two dither strokes of
different colours over the same spot, with the flip toggled between them,
together cover every pixel — split evenly between the two colours, an
interlaced pseudo-mix without ever blending the actual colour values. Because
the flip changes over time, it's baked into each save-file stroke record
(`kind, flip, colour, size, x1,y1,x2,y2` — 19 bytes) so a reload replays
identically to what was drawn, not with today's flip.

**Verification status: full Lua flow (fit → build canvas → build all six
buttons → simulated button taps → simulated canvas strokes → save → load)
runs clean with zero errors in the `render-tui` backend. Not run in the
headed backend (no display in this sandbox), and TUI pixel output wasn't
usable as visual proof — see the camera-math caveat below.** Getting a clean
run required finding and fixing two real, pre-existing engine bugs, plus
tracking down a third (already-known, still-open) one to its actual trigger
condition — none of these three were caused by `apps/art`'s design, only
surfaced by live-running it the way this repo's QA methodology requires:

1. **The TUI backend never drained `loggy`'s error channel.** Lua runtime
   errors during `loop()` are sent via `ctx.loggy.send(...)`
   (`src/lua_define.rs`) into a channel that only gets drained by
   `Loggy::listen()` — called from exactly one place in the whole codebase,
   `src/gui.rs:634`, itself `#[cfg(feature = "headed")]`. The `render-tui`
   backend (`src/tui/mod.rs`) never called it at all, so a throwing `loop()`
   in TUI mode rendered nothing, logged nothing, and looked indistinguishable
   from an app that was simply idle. Fixed by adding `core.loggy.listen();`
   to the TUI tick loop. Without this fix, everything below would have been
   invisible — the app would have just sat on a blank screen forever with no
   diagnostic.
2. **`tex()` always hung/errored under any non-headed backend.** Its native
   (`src/command.rs`) blocks on `rx.recv()` waiting for a `MainCommmand::SetImg`
   ack; the handler in `src/lib.rs` sent that ack from *inside* the
   `#[cfg(feature = "headed")]` block, so under `render-tui` (or any
   non-headed build) `tx` was simply dropped, unacked, at the end of the
   match arm — closing the channel. Lua then saw "receiving on a closed
   channel" rather than a hang, but the practical effect was the same: no
   non-headed backend could ever call `tex()`, which every texture-uploading
   app (`apps/paint` included) does. Fixed by moving the ack send outside the
   `headed` gate (a no-op ack when there's no GPU to upload to — same
   precedent as the audio natives compiling to no-op stubs without
   `--features audio`, see CLAUDE.md).
3. **`Invalid chunk due compilation corruption`, hit inside `build_buttons()`.**
   Initially looked threading-related (it only ever appeared on a *second*
   `tex()`-calling function, and the reported line pointed nowhere near
   anything wrong — consistent with fix #2 above being a recent unblock).
   It wasn't: this is **SILT-BUGS.md #5**, an already-documented, still-open
   silt-lua compiler bug — a table constructor holding a local, written
   *inside a loop body*, corrupts the compiled chunk. Bisected with `cout`
   tracepoints between every statement of the loop body down to the exact
   line: `ent.size = { size, 0.08, size }` — a **field assignment**
   (`obj.field = {...}`), not the `t[#t+1] = {...}` bracket-indexing case
   SILT-BUGS.md #5 originally documented. Same root cause, wider trigger
   than previously known — added that finding to SILT-BUGS.md #5 directly.
   Fixed with the already-documented workaround: hoist the constructor into
   a local first (`local sz = {...}; ent.size = sz`), applied everywhere in
   `build_buttons()`/`update_selection_marks()` that had the shape.

With all three addressed, `apps/art`'s full simulated flow (six-frame harness
driving fake pointer/click events through fitting, button taps, a canvas
stroke, save, and load) completes with no errors logged. What's *not*
verified: the TUI backend's rendered pixels never showed the scene's
geometry in this session's captures. A quick check ruled out "nothing
renders at all" — `apps/spin` still rendered correctly after the engine
edits above, and a debug marker cube placed along +X with `rot={0,0}` did
render — but `apps/art`'s own camera convention (`rot={tau/4, 0}`, meant to
look down +Y per FRESCO's comment and this session's math for
`src/tui/renderer.rs::camera_matrix`) did not show the scene in this
backend, and swapping the rotation value didn't change the pixel count
either, which doesn't fit a simple sign/axis mixup. Not chased further:
pixel-exact TUI camera verification is disproportionate effort for an app
whose real target (mouse-driven 3D unprojection painting) is the headed
backend, which this sandbox can't display. Worth a real check in headed mode
before trusting the on-screen result blind.

---

## Cross-cutting checks run after every change in this doc

- `cargo build --features audio` clean.
- `cargo test --features audio --lib` — 39/39 passing as of this writing.
- `cargo check --no-default-features --features silt,render-tui` — was **11**
  pre-existing errors (HANDOFF.md says 12; see its note on that discrepancy),
  fixed to **0** by the TUI-restart work above; re-verified at 0 again after
  the `apps/art` session's `loggy`/`SetImg` fixes. A new error here means a
  regression.
- Wasm (`--target wasm32-unknown-unknown --features wasm`) is **not**
  buildable in this sandbox at all (pre-existing `getrandom` crate/toolchain
  issue, unrelated to anything in this doc) — the `lum{}` wasm bridge
  (`worker_protocol.rs::VmToHost::Light`, `command.rs::main_command_to_host`)
  was kept textually consistent with the native side but has **not** been
  compiled or run for wasm by anyone.

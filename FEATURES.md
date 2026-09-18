# Feature checklist

Status of the recent feature wave (lighting/shadow/fog, the collision system,
the terminal renderer backend, sound transport, chisel, and the new example
apps) plus the reset/race fixes found while dogfooding it. This is **not**
the whole engine's 700+ commit history — just what's been added/touched in
this collaboration and is still short on regression coverage, per your note
that this needs tracking better.

Three checkboxes per item:
- **Complete** — the feature does what it's meant to, in normal use.
- **Tests added** — there's an automated test (`#[test]`, not a manual
  smoke-run) exercising it.
- **Coverage good** — those tests cover the feature's real edge cases, not
  just the happy path.

Where I couldn't personally verify something this session (no display in
this environment, or just never re-checked it), **Complete is left
unchecked** rather than assumed — please confirm or correct those.

## Legend
`[x]` done · `[ ]` not done / unconfirmed

---

## Rendering & lighting (L0–L2)

| Feature | Complete | Tests added | Coverage good | Notes |
|---|---|---|---|---|
| Directional "sun" lighting (`lum{}`, default shape) | [x] | [ ] | [ ] | Shader math (`compute_shade`) has no Rust-side test; only ever checked visually. |
| Hemisphere ambient (`lum{sky=...}`) | [ ] | [ ] | [ ] | Not re-verified this session — please confirm it still looks right. |
| Cone / sphere point-and-spot lights (`lum` shape 1/2) | [ ] | [ ] | [ ] | Not exercised by any example app yet; unverified. |
| Shadow map (`shdw()`) | [ ] | [ ] | [ ] | Never tested by me this session — no app uses it. |
| Gouraud shading toggle (`gour()`) | [ ] | [ ] | [ ] | Same — no app exercises it yet. |
| Distance fog (`fog{}`) | [x] | [x] | [ ] | `Global::distance_fog_does_not_leak_into_the_next_app` covers the per-load reset. The deeper stale-message race (below) has no automated test — hard to without a threaded test harness. |
| Chip presets (R00 / R43 / R30) | [x] | [x] | [ ] | Header-byte parsing well tested (`asset.rs`, 5 tests). `apply_chip`'s actual per-preset values (filter mode, crt_resolution, etc.) aren't asserted anywhere. |
| Monitor presets (LCD / Slot / Grille) | [x] | [x] | [ ] | Same as chip — parsing tested, applied values aren't. |
| Vertex-colour tint (`ent.tint`) | [x] | [ ] | [ ] | Fixed this session (tile chunks defaulted tint to transparent black instead of opaque white — see fixes below). No regression test pins the default. |
| Engine fallback texture (untextured model/tile → checker) | [x] | [x] | [x] | `texture.rs::the_fallback_texture_owns_the_first_atlas_slot`. |

## Collision & world

| Feature | Complete | Tests added | Coverage good | Notes |
|---|---|---|---|---|
| AABB + cylinder collider math (`collide.rs`) | [x] | [x] | [x] | 10 unit tests: box-box, cyl-cyl, cyl-box, AABB construction. Solid. |
| `hit.cell()` / `hit.grid()` / `hit.all()` natives | [x] | [ ] | [ ] | Exercised manually via `apps/guys`; no Rust-side test calls these end-to-end. |
| Per-entity collider override (`hit_shape`/`hit_size`/`hit_offset`) | [x] | [ ] | [ ] | Same — works in practice (`apps/guys`), no test. |
| Entity grid (`tile_grid.rs`, backs `hit.grid`) | [x] | [x] | [x] | 8 unit tests. |
| `tile(asset,...)` falling back model → texture → checker | [x] | [ ] | [ ] | Confirmed working in `apps/guys` (ground tile textured via a bare `tex()` name); no automated test. |

## Sound

| Feature | Complete | Tests added | Coverage good | Notes |
|---|---|---|---|---|
| Sample-clock transport (`bpm()`/`cue(...,at_beat,quant)`) | [ ] | [ ] | [ ] | Committed (`3a14df2`) before this session; I haven't verified it since. |

## Tooling

| Feature | Complete | Tests added | Coverage good | Notes |
|---|---|---|---|---|
| Chisel grid modeller | [ ] | [ ] | [ ] | Committed (`c96bddc`) before this session; not touched or re-verified here. |

## Terminal (TUI) backend

| Feature | Complete | Tests added | Coverage good | Notes |
|---|---|---|---|---|
| Software rasterizer render backend (`src/tui/renderer.rs`) | [x] | [ ] | [ ] | You confirmed perspective matches the wgpu path. No automated test (would need pixel-comparison harness). |
| Keyboard input, held-key tracking | [x] | [ ] | [ ] | Fixed this session (no real Release event without Kitty protocol → key never depressed; added a last-seen timeout). Compiles + smoke-tested for zero errors, but the actual timing (400ms) is **not confirmed by you yet** in a live terminal. |

## Reset / cross-thread correctness (found + fixed this session)

| Feature | Complete | Tests added | Coverage good | Notes |
|---|---|---|---|---|
| `attr{}` state resets on every app load (not just hard/soft reset) | [x] | [x] | [x] | Pre-existing fix (`a4475b2`), `Global::app_attrs_do_not_leak_into_the_next_app`. |
| Lighting/fog resets on every app load, not just `clean()` | [x] | [x] | [x] | This session's fix, `Global::distance_fog_does_not_leak_into_the_next_app`. |
| Stale cross-bundle messages drained after `hard_reset` | [x] | [ ] | [ ] | `drain_stray_messages()`, called at every `hard_reset` site (cold boot ×3, and the `load`/`exit`/`reset`/`new` console commands). No test — would need a fake bundle + timing race to simulate, which is exactly the kind of thing that's easy to get "looks right" and hard to prove. |

## Example apps

| App | Complete | Tests added | Coverage good | Notes |
|---|---|---|---|---|
| `apps/spin` | [x] | [ ] | [ ] | Minimal rotation sanity check; manually run in TUI. |
| `apps/rig` | [x] | [ ] | [ ] | Texture/swatch smoke test; manually run. |
| `apps/art` | [x] | [ ] | [ ] | Manually run in TUI only (no display here for the headed path). |
| `apps/guys` | [x] | [ ] | [ ] | Full character controller (movement/gravity/jump/NPC AI/floating platform). Verified only by headless TUI runs watching for zero Lua errors over several seconds — never visually confirmed by a human until you checked the black-tile/fog issues. |

---

## The honest summary

- **Automated coverage is real but narrow**: it's concentrated in pure-data/math
  modules that are easy to unit test in isolation (`collide.rs`, `tile_grid.rs`,
  `bundle.rs`, `asset.rs`'s header parser, the two `Global` reset tests). Almost
  nothing that touches rendering, the Lua native surface, or cross-thread timing
  has a test, because those are the hard/expensive ones to write, not because
  they matter less.
- **Every "found + fixed" bug this session** (tile tint default, fog reset gap,
  stray-message race, TUI key release) was caught by manual runs or by you
  actually using the thing — not by a regression test catching a regression.
  If any of these regress silently, nothing here will notice.
- **No integration/end-to-end test exists for any example app.** "Manually run
  in TUI, watched for zero error lines" is smoke-testing, not a regression
  suite — it proves the app didn't crash *that run*, not that its behavior is
  correct or stays correct.

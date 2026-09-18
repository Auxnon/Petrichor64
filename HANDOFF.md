# Handoff — Petrichor64

For the next agent picking this up. Written after a long session spanning the overlay
tool suite, the entity refactor, sound timing, and a lot of silt debugging. Read
`CLAUDE.md` first (it is the authority on conventions); this covers what isn't in it.

Branch: `copilot/sub-pr-6`. Only uncommitted file is `web/default.game.png`, which was
already modified when I arrived — leave it alone unless the owner says otherwise.

---

## 1. Build and verify

```
cargo build --features audio          # the native build you want 99% of the time
cargo test --features audio --lib     # 14 tests
just check                            # native, native+audio, headless-tui, wasm32
just check-android                    # needs only `rustup target add aarch64-linux-android`
cd synth && cargo test                # 11 synth tests (the audio DSP lives here)
```

- **`just check` fails and always has.** The `--no-default-features --features
  silt,render-tui` line is broken with **12 errors** (missing `Gui` fields, `Core::gfx`,
  `winit`). That is the baseline — if you touch shared code, re-run it and confirm you
  are still at 12, not 13. `just build-headless` is broken for the same reason.
- **Never commit `test/sound/scripts/main.lua`.** Standing instruction from the owner;
  it is his scratch file.
- Commit messages end with
  `Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>` and carry a
  `tickets[no-tickets]` marker (a push hook rejects commits without a `tickets[…]`).

### silt is a sibling repo you will end up editing

`silt-lua` is a **path dependency** at `../silt-stable`, with the `vector` feature
enabled from petrichor's `Cargo.toml`. It has its own agent. Conventions there differ:
commits are `fix: [aid] …` / `perf: [aid] …` with **no** `tickets[…]` marker. Its
`cargo bench` targets are excluded from `cargo test`. Coordinate before large changes;
small measured fixes have been welcome.

---

## 2. Architecture, in the parts that bite

**Bundles.** A bundle is one Lua VM + its own gui raster, entities, world layer and
pool. `BundleManager` owns them in an `FxHashMap<u8, Bundle>`; ids come from
`next_free_id()` (lowest free slot — it used to be a monotonic `u8` that wrapped onto
the running app after 256 overlay opens). `layer_order()` is the canonical draw/priority
order: non-overlays first, then overlays, and the renderer and input owner both read it
every frame rather than being told when to rewire.

**Overlays** are the privileged surface — filesystem access via `app.*`, input
ownership, their own gui layer. The trust boundary is the thing to not break:

- `app.*` is **built only** for a bundle the engine marked as an overlay, so in a
  game's VM the natives do not exist. That forces the mark to be set *before* the VM
  starts, which is why overlays load through `command::load_overlay`, never
  `load_app` + `mark_overlay` after the fact.
- Nothing in Lua can create an overlay. An app's `over()` passes its bool as
  `is_parent` and makes an ordinary child bundle — similar name, unrelated path.
- Every `app.*` packet is re-checked against its sender on the main thread via
  `Core::overlay_edit_target`, which is also the only place the edit target is
  resolved (first non-overlay bundle, so two editors both aim at the game).

**The gui layers** composite by **alpha test, not blend** (`if (system.a < 0.1)` picks
the topmost opaque-enough layer), so an overlay *cannot* dim the app behind it without
a real `mix()` in `gui_fs_main`. There are four layers and one is the console, so at
most three concurrent surfaces. `sync_layer_targets` is the single writer of
`bundle_target`; `mark_bundle_dirty` only sets the dirty bit. Do not let anything else
assign a layer's target — that bug (two writers) caused an overlay to hijack the app's
layer every frame.

**Rasters are published, not shared.** `LuaImg` holds the live `image` plus a
`presented` copy. The owning Lua thread calls `publish()` at the end of its loop and
the renderer uploads only `presented`. Reading `image` directly used to catch ~10% of
frames mid-repaint (after `clr()`, before anything was drawn back) which read as
stutter. If you add another consumer of a Lua-owned buffer, follow the same shape.

**Entities are per bundle** (`EntManager.bundles: FxHashMap<u8, BundleEnts>`), each with
its own array, `render_hash` and dirty flag. `check_ents` returns one batch set per
bundle and `render.rs` walks them in `layer_order()`. Ownership is stamped from the
*packet sender* in `create_from_lua`, not from `LuaEnt.bundle_id`, because that field
is hardcoded to 0 and nothing else sets it.

**Sound** lives in the `synth/` crate, native-only behind `--features audio`. One cpal
stream owns all state on the audio thread; the engine talks one-way over an mpsc
`Sender<SoundCommand>`. The mixer is a `move || -> f32` closure called **per sample**
that drains commands every sample. There is now a transport: a monotonic frame counter
plus a shared `bpm`, with `PlayAt(note, ch, at_beat, quant)` scheduling against it.
Beats convert to frames *on the audio thread* so tempo has one owner.

Audio-thread rules (from `CLAUDE.md`, and they are real): no allocation or free in the
callback. The scheduling queue is a fixed 64-slot array of `Copy` notes for that
reason, and `Note`'s consonant clusters are a fixed-size `Cluster` rather than a `Vec`
for the same reason. **Anything persistent you add must be cleared in the `Reset` arm**
or it leaks across app loads.

---

## 3. Invariants that will bite you

1. **`gunit`: integer = pixels, float = percentage.** A stray `32.0` where you meant
   `32` places something a third of the way across the screen. Wrap computed
   coordinates in `flr()`.
2. **Glyphs blit with transparency.** Drawing a second colour over the same text cell
   blends with the first instead of replacing it — colour runs must be drawn once each
   (see `paint_line` in `apps/edit`).
3. **Modifier keys have exactly four slots.** `bit_check` folds left/right onto
   247–250, so the names that work are `alt`/`ctrl`/`shift`/`win` (and now the
   left/right aliases). A test in `controls.rs` pins `key_match` against
   `keycode_to_index` — keep it passing.
4. **`cin()` returns unshifted characters** and reports a char whether or not ctrl is
   held, so chorded keys must be handled before typing or `ctrl+s` also types an "s".
5. **`frame_split` is a divisor** — 1 means every frame. The gate is
   `skips >= frame_split.saturating_sub(1)`; comparing bare `>=` halves every bundle's
   tick rate.
6. **`make()` returns a handle for a model that does not exist.** It is worthless as
   evidence that `mod()` built anything — check `gmod("name")` instead.
7. **`mod()`'s `t` is optional**; an untextured model gets the engine's fallback
   checker, which `TexManager::reset` installs into atlas slot 0 and which survives
   `rebuild_atlas`. An app shipping its own `default` image overrides it.
8. **`attr` state must be reset per app load** in `Global::clean_app_attrs`. It only
   cleared the console lock, so the boot logo's `attr{fog=200}` leaked into every app
   that followed.

### silt gotchas (see `SILT-BUGS.md` for the full write-ups)

- **#5 is open:** a table *constructor* containing a **local**, assigned into
  `t[#t + 1]`, corrupts the whole compiled chunk. Use `table.insert(t, {...})`.
  Constants in the same position are fine, which is why small tests miss it.
- No `string.gmatch` — split with a `find` loop.
- Error line numbers point at whatever ran *next*, not at the bad code. Bisect by
  stubbing functions (a later definition of a global wins, so you can append an
  override at the end of a file to disable something).
- `t[#t] = nil` does **not** shrink `#t`.
- Field access costs ~64 ns against ~15 ns for a local op. One method call writing
  three fields beats three field writes by 1.77×. `benches/userdata.rs` in silt has
  the breakdown.

---

## 4. How to actually verify things here

You cannot see the screen and you cannot press keys. What worked:

- **Lua tick rate:** an app that `cout`s every N loops, run for a fixed window against
  the *prebuilt binary*. Never time through `cargo run` — build time lands inside your
  measurement window (that mistake produced a bogus "10 Hz overlay" claim).
- **Startup phases:** pipe stdout through a python filter that prefixes each line with
  elapsed time. That found 1.06 s of dyld load before `main()` and a 5 s deadlock
  timeout that looked like work.
- **Anything sub-millisecond:** criterion in `synth/` or `../silt-stable/benches/`. A
  whole-engine harness here could not resolve a 2% effect — its own control drifted
  24% between runs.
- **Always include a control** that must *not* move. It caught a false positive twice.
- **Input-driven apps:** append a test driver to a scratchpad copy of the app that
  calls the functions directly (`base_loop = loop` then redefine `loop`). That is how
  the editor's insert/newline/backspace/save and the modeller's extrude were verified.
- **Audio timing:** step the mixer sample by sample and find the first frame above a
  threshold (`first_sound_at` in `synth/src/sound.rs`).
- Use the scratchpad for throwaway apps; note that an app **needs an `assets/`
  directory** or it fails to load.

Two process traps I hit: a `python` string-replace can fail silently and leave you
reporting a fix that never applied (**assert on every replacement**), and **zsh does
not word-split unquoted variables**, so `cargo check $flags` passes one giant argument.

---

## 5. In flight

**Physics** — designed with the owner, not started. Agreed shape: roll our own (AABB +
swept AABB against the voxel grid; rapier is the wrong shape and costs megabytes
against an `opt-level="z"` release profile). `hit.*` and per-entity `ent1:hit(ent2)`,
**plus a batch query** so the O(n²) loop lives in Rust — at ~64 ns per field crossing,
30k crossings a frame is ~1.9 ms of pure overhead. `LuaEnt` already has `size`,
`offset` and `vx/vy/vz`; check what `size`/`offset` are currently used for before
repurposing. Ent-vs-tile matters more than ent-vs-ent, and the chunk grid is the broad
phase for free. Bones ≠ Verlet: skinning (vertex weights + a bone matrix palette) is a
renderer change and is the thing actually needed for non-baked models; Verlet only
helps once a skeleton exists.

**Overlay 3D** — camera ownership landed (`Bundle.cam: Option<BundleCam>`;
`has_camera` is the switch: no camera means it borrows the app's camera and depth,
which is free; calling `cam` opts into its own pass). The pass itself is scoped in
`PLAN.md`: group instance buffers per bundle (done), a second camera in the uniforms,
and the pass between the app's 3D and the gui composite.

**Sound** — the clock landed (`bpm`, `cue`). Next, in order: per-channel looping (watch
out — replacing a loop drops the old `Vec` *on the audio thread*, same class as the
`Chain` shape already there); sample slicing on `smpl` (offset/length, loop points,
reverse, rate independent of pitch) which with the clock is most of jungle; then a
2-op then 4-op FM instrument variant for Genesis timbres. Soundfonts were deliberately
dropped — if multi-sampling comes back, **SFZ** (plain text pointing at ordinary
samples) is the size-conservative option, not SF2.

**Vibrato** is an LFO on pitch, gated to sung notes, configured per channel — it does
*not* share the envelope's staging, which is per-voice and per-instrument. The owner
was last asking about unifying them; a general modulation source (so depth can be
enveloped) buys delayed vibrato onset and pitch envelopes for toms/basses.

**Known open, not mine to close:** the modeller still renders wrong on screen ("still
broken" per the owner, paused). The packer does not bundle `sounds/` into
`.game.png`, so sample-based music cannot ship packed. `nil is not callable@0:0` after
every resize. `versionCode` derives oddly (16778243) from 0.4.3.

---

## 6. Working practices

From the owner's Everon DevOps law (in `~/.claude/`), the parts that apply here:

- **Green it locally before you push.** Derive the blocking checks from the repo and
  iterate until clean. "It'll probably pass in CI" is not done. Failures block;
  warnings are tolerated but read them.
- **Audit a plan before presenting it.** Any multi-step plan gets a second pass from a
  fresh subagent on the strongest planning model — quietly, as a gate, not a
  deliverable. (Note: this session had a standing instruction *not* to use subagents
  unless asked; check which rule is live before spawning one.)
- **Every change is a ticket** and the link is the `tickets[…]` **commit marker** —
  not the branch name, not the MR description. `dev`/`main` are protected; promote by
  MR. Set only the early statuses (Discovery / In Progress / Code Complete / Blocked);
  the pipeline owns the rest. Call `get_workspace_context` before other backlog MCP
  calls. This repo uses `tickets[no-tickets]`, so the marker matters more than the
  ticket here.

What this session taught, worth carrying:

- **Measure before asserting, and report the measurement.** Several confident
  diagnoses of mine were wrong — that the userdata *mutex* was the cost (it is 7%),
  that dependency optimization would speed startup (no gain, 6× slower rebuilds), that
  an overlay ticked at 10 Hz (a measurement artifact). Every one was caught by
  measuring, and each correction was cheap because nothing had been built on it yet.
- **Say plainly when a claimed fix did not land.** I twice reported a fix whose edit
  had silently failed. Correcting it immediately costs a sentence; leaving it costs the
  owner's trust in every other claim.
- The owner asks sharp architectural questions and is usually right — the starvation
  objection to my in-flight-skip idea killed a design that measured 216 deferrals to 1
  upload. Take the pushback seriously and go measure it.
- Commit messages here carry the *reasoning and the numbers*, not just the change.
  They have been the most useful record in the repo; keep that up.

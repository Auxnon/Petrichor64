# Petrichor64 — notes for agents

Building a game or app *with* the engine (not changing the engine)? Read
`AGENTS.md`: the silt-safe Lua style, mesh/camera/picking recipes, and how to
playtest headlessly with a `TEST_HOOK` on a hidden monitor.

## Lua command naming (core `lua!` natives in `src/command.rs`)

Core Lua command names should be **3–4 letters**. This keeps the scripting API
terse and console-friendly (it's a fantasy-console engine).

- **Prefer a real word** at 3–4 letters: `cam`, `key`, `tex`, `note`, `tile`,
  `make`, `mute`, `lamp`, `fog`, `song`, `anim`, `attr`.
- **Fun / rootword shortcuts are fine** when they read well: `mus` = mouse
  (Latin *mūs*), `flr` = floor, `rnd` = random, `img` families, etc.
- If you genuinely can't find a good 3–4 letter name, make it **as short as
  possible and get the user's approval before adding it** — don't ship a long
  name silently.

New commands: add the `lua!("name", …)` native in `src/command.rs` **and** a
matching `guide/<name>.md` (the macro `include_bytes!`s it in debug builds, so a
missing doc fails the build).

Long names predating this rule (candidates to shorten with approval): the
tile family `gtile`/`ftile`/`dtile`/`istile`, plus `chord`, `instr`, `empty`.

## World axis convention

This engine is **Z-up**: x,y are the ground plane, z is height. (See
`compute_shade` in `shader.wgsl` — the hemisphere-ambient mix uses the
surface normal's `z` component as "up".) An example app building terrain
should place height along `z`, not `y` — mixing the two makes the world
render sideways.

## `tile()` accepts a bare texture, not just a model/asset name

`tile(asset, x, y, z, rot)` first looks for a model matching `asset`; if none
exists, it looks for a **texture** of that name and — if found — auto-builds
a cube tile textured with it on every face (see `guide/tile.md`). So a
terrain tile doesn't need a dedicated model or a call to `mod()`: registering
a texture with `tex(name, image)` and then calling `tile(name, ...)` is
enough. An asset that resolves to neither a model nor a texture still places
a cube, textured with the engine's fallback checker.

## Sound & audio (`src/sound.rs`, natives in `src/command.rs`)

Audio is a **native-only, non-default feature** (`--features audio`, pulls
`cpal`). All sound code and the sound `lua!` natives are `#[cfg(feature =
"audio")]`; the natives compile as no-op stubs otherwise (hence the many
"unused variable" warnings in the no-audio build — expected, not a regression).

Architecture: a single cpal output stream owns all synth state on the audio
thread; the engine talks to it one-way over an `mpsc` `Sender<SoundCommand>`
(`core.singer`). The stream **outlives Lua reloads**, so state must be reset
explicitly — `SoundCommand::Reset` (sent from `async_load_app` on every load)
clears instruments, samples, the loaded-file bank, and all voices. If you add
persistent audio state, clear it in the `Reset` arm too.

- **Instruments and samples are keyed by integer id — keep it that way.** The
  mixer hot path is a fast index lookup (`FxHashMap<usize, …>`); do **not**
  move it to string keys. Names only exist in the load/bind staging.
- **`instr(id, spec, cfg?)`** — oscillator (waveform-name string) or additive
  (harmonic-amplitude table). **`smpl(id, data, cfg?)`** — sampler: `data` is
  either a PCM table (raw `-1..1`) or a **name string** binding a loaded file.
- **Envelope lives on the instrument, not the note** (keeps note/chord/song
  terse with `instrument` trailing). The `cfg` table on `instr`/`smpl` carries
  the ADSR (`atk`/`dec`/`sus`/`rel`) plus the type scalar (`wid` pulse duty /
  `base` sample pitch). A bare number `cfg` is the legacy scalar. Defaults
  reproduce the old fixed 4ms/12ms envelope. Parsed by `parse_sound_cfg`; the
  synth runs a per-voice ADSR state machine (`EnvStage`/`Voice::advance_env`).
- **Master bus**: `master_volume` into a `tanh` soft limiter (not a hard clamp),
  so single notes stay loud while dense polyphony compresses instead of clipping.
- **Waveforms** live in `WaveType` + `osc()`; **samples** go through
  `voice_out()` (pitch-resampled by `freq/base_freq`, linear interp, one-shots
  self-release). New sample buffers are loudness-matched via `normalize_pcm`.
- **Silt `Table`→`Vec` used to have a hash-order bug** (`to_vec` walked the
  hashmap). Fixed in silt PR Auxnon/silt-lua#16 — it now reads the 1..n sequence
  (`ipairs` semantics: stops at the first hole, ignores non-integer keys).
  The existing `getn(i)` loops for PCM/chord/song, and `ordered_list` in
  `command.rs`, are still correct and safe to keep.

### Loading sound files (`sounds/` folder)

Games may have a `sounds/` folder alongside `assets/`/`scripts/`. `.ogg` files
there are decoded at boot (pure-Rust `lewton`, no C deps), normalized, and
stashed in a name→PCM bank keyed by filename stem (extensionless, like
textures/models). `smpl(id, 'name')` binds one into an integer slot — the
string lookup happens once, at bind time, never in the mixer. Buffers are
`Arc`-shared so binding one file to several slots doesn't copy.

The engine only **decodes** ogg. Converting `.wav`/`.mp3` → `.ogg` is the job
of the separate **`oggify`** CLI crate (keeps the Vorbis *encoder*'s C
dependency out of the size-optimized engine binary). See PLAN.md → Sound System.

Wiring lives in `src/asset.rs` (`decode_ogg`, `load_sounds_from_dir`,
`load_sounds_from_buffers`) and the load funnel `command.rs::async_load_app`.
Directory games load from disk; packed `.game.png` games decode the bundled
`sounds/` entries in `asset::unpack` (whitelist `["assets","scripts","sounds"]`).
**Known gap:** the *packer* (`collect_packable_sources`/`pack_folder`) does not
yet bundle `sounds/` into `.game.png`, so packed-game sound loading is untested.

## Meshes from tools: `mod()` polygon/line forms and `glb()`

Besides the `v`/`i`, `q` and cube forms, `mod()` takes a **polygon form** (`p`
points, `f` faces as **1-based** index lists, `fu` one uv per face) and a **line
form** (`l` endpoint pairs, `lw` width(s), `lu` one uv per segment), both built in
Rust by `gltf_io::MeshData::{from_polygons, from_lines}`, and both with an optional
`scale` (model vertices pack at 1/16 unit — build fine meshes larger and scale the
entity down). Use these for anything rebuilt often: Lua-side triangle generation
was ~10x slower. `glb(path, data)` writes the same tables (or `v`/`i`) as
`.glb`/`.gltf`, `glb(path)` reads one back; see `guide/glb.md`. The modeller that
drove these lives outside the repo at `~/projects/model-chor`.

## `mod()` and `tex()` don't wait for the main thread

They used to block on a rendezvous ack, which cost a full main-loop pass per call.
Commands from one Lua thread reach the main thread in order over one channel, so
a following `make()`/`gmod()`/`mod()` already sees the result; the ack slot is now
buffered and unread. If you add a native that must observe main-thread state,
give it its own reply channel and wait on that — don't reintroduce the ack here.

## Parenting and skeletons (`lot`, `ent_manager::Tree`, `skeleton.rs`)

`lot` writes `parent`/`bone`/`wear` onto the child `LuaEnt` on the Lua thread;
there is no main-thread command and no ordering of the entity list. Each frame
`check_bundle_ents` builds every entity's own matrix, then `Tree` resolves
parents, bone frames (`Skeleton::attach`: joint origin, bind-pose axes), worn
poses (`Skeleton::worn`, by joint name) and `ik` on demand, memoized. On the web
these fields ride `EntLook`/`EntProps`; anything new that changes how an entity
is placed has to be added to `EntLook` or the mirror never hears of it. A pose
is `SkelState::pose(skel, now, world)`; `world` is needed because `ik` goals are
world points. `ent:bone()` on the Lua thread only knows the entity's own
matrix, so it's off for an entity that rides a parent.

## Function-scoped reload (`app.swap`)

`Core::app_swap` hands the old and new source of one script to the app's Lua
thread (`LuaTalk::Swap`), which calls silt's `hotswap_functions`: changed
function bodies are replaced in place (globals rebound with their upvalues
kept, nested prototypes redirected), and anything else comes back as "root
changed" with nothing run, which `app_swap` answers by reloading the app.
`Core::live_src` remembers what each script's VM is really running, since the
file on disk may be newer; `command::reload` clears it for the bundle.

The swap is given the script's file name as its chunk name, while the loader
names chunks by full path. `make` stamps entities with the file name only
(`LuaEnt::origin`, from `vm.call_site()`) so both agree.

silt's `hotswap` (no `_functions`) would rerun the file's top level over the
live globals instead of reloading. It isn't used: an app's `main()` wouldn't
run again, so entities and tiles made there would be stale.

`command::reload` (and so a `"root"` swap) resets the camera and clears
**every** model (`ModelManager::reset`), an open overlay's included, and
repacks the atlas. An overlay with 3D of its own has to rebuild after it
(edit-chor's `pane_restore`).

## Built-in tools (`src/builtin.rs`, `build.rs` `editors`)

image-chor (Ctrl+P), model-chor (Ctrl+M), edit-chor (Ctrl+E) and sound-chor
(Ctrl+N) are separate repos compiled in as overlays. Adding one is an entry in
`editors::ALL` (build.rs), a module and match arm in `builtin.rs`, a
`*_OVERLAY` constant and toggle in `command.rs`, and a key in
`controls_evaluate`. Only `scripts/` ships: no assets, no sounds.

## Scripts load in sorted order

`asset::get_script_items` sorts, so every file's top level runs in the same order
on every filesystem. Still, keep top-level code free of other files' globals
(compute those in `main()`): packed games and future loaders needn't sort.

## The engine is a library; `chor` is one host (`src/host.rs`, `src/instance.rs`)

`Instance` is the engine without a window: one cart, its VM threads, a `Core`.
A host hands it `Input`, calls `frame(now)` / `render()`, samples `texture()`,
and takes `requests()`. `App` in `lib.rs` is the winit host `chor` uses: it
translates `WindowEvent`s to `Input` and grants every `Request` (`App::grant`).
`examples/two_carts.rs` is a second host; `tests/host.rs` runs two instances on a
windowless device.

- **Nothing below `App` may touch a window.** `Gfx` has a `Target` (a window's
  surface, or a texture on the host's device) and no window calls: title,
  fullscreen, size, pointer grab, quit and clipboard are `Request`s
  (`core.requests`, `Gfx::request`). A new window-ish feature is a new `Request`
  variant plus an arm in `App::grant`.
- **Input goes through `Instance::input`**, not into `core.global` from `App`.
  Positions are the instance's own physical pixels.
- **No process-wide state for anything a cart can see.** Two instances share a
  process: skeletons and the clock live in `skeleton::Rig` on the `Core`. What's
  still process-wide: `gpu_loss.rs` (only `chor`'s own device is watched; a
  host's device is the host's to watch), `midi.rs`, and the synth's
  `SOUND_STATE` behind `sounds()`.
- **`Policy` is enforced by construction** in `init_lua_sys` (a forbidden global
  is never registered) and again by `MainCommmand::allowed_by` on the main
  thread. A new native that reaches outside the instance needs both.
- A hosted instance's texture is remade at once on `resize` (the host
  re-binds it); a window's resize stays debounced.
- `Instance::new` is native-only: on the web the VM runs in a worker that `App`
  drives, and a page gets several engines by starting several module copies.

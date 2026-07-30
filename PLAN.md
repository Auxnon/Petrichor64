# Petrichor64 Rendering Diagnostic Plan

## Confirmed Working
- Simple triangle shader (test.wgsl) draws correctly — the pipeline infrastructure, surface creation, and swapchain presentation work.
- Clear colour (currently violet) is visible — the render pass itself executes.

## Root Causes Found (wgpu 0.15 → 24 migration)

### 1 — GUI / Sky textures never uploaded to GPU **(Critical)**

**Symptom**: Only the clear violet colour is visible; no sky, console text, or Lua-drawn GUI.

**Cause (three-part chain)**:

a. In `src/lua_define.rs` (Lua loop), `BundleMutations::new()` defaults to
   `gui: true, sky: true`, but two lines immediately override both to `false`:
   ```rust
   let mut mutations = BundleMutations::new();
   mutations.gui = false;   // ← always false
   mutations.sky = false;   // ← always false
   ```
   The proper dirty-detection code that was supposed to re-enable these flags
   was commented out and never re-implemented for silt-lua.
   Because mutations are always false, `Core::update` never calls
   `gui.mark_dirty()`, so `ScreenLayer::dirty` remains `false` after the first
   successful `check_render` call.

b. `pool.gui_dirty` / `pool.sky_dirty` (the `Arc<AtomicCell<bool>>` in
   `SharedPool`) are initialised to `false` and nothing ever sets them to
   `true`. `ScreenLayer::check_render` gates the actual `write_tex` upload on
   `pool.gui_dirty.load()`, so even if the layer *was* dirty, the texture is
   never written to the GPU.

c. `check_render` resets `self.dirty = false` whenever the pool exists,
   regardless of whether the texture was actually uploaded. This prevents any
   future upload attempt even after `mark_dirty` is later called.

**Fix**:
- Remove the `mutations.gui = false; mutations.sky = false;` overrides in
  `lua_define.rs`, restoring the `BundleMutations::new()` defaults of `true`.
  This ensures `mark_dirty` is called on every `LoopComplete`.
- Simplify `check_render` to upload the LuaImg unconditionally whenever
  `self.dirty` is true and the LuaImg weak-ref is upgradeable, removing the
  `pool.gui_dirty` gate. Reset `self.dirty = false` only after a successful
  upload.
- The `pool.gui_dirty` / `pool.sky_dirty` fields can be re-wired later for a
  proper per-frame dirty optimisation (see §5 below).

---

### 2 — System / Console layer never shows console text **(Critical)**

**Symptom**: Even when the in-engine console is open, no text is visible.

**Cause**: `Gui::apply_console_out_text()` draws text into
`ScreenLayer::image` (an `Arc<AtomicCell<RgbaImage>>`). But
`ScreenLayer::check_render` ignores `self.image` entirely — it reads from the
Lua-owned `LuaImg` via the pool weak-ref. These are completely separate
objects. The console pixels are never uploaded to the system GPU texture.

Additionally, for `ScreenIndex::System`, `check_render` uses `pool.gui.as_ref()`
which points to the **primary Lua raster**, not the system/console raster.

**Fix**: In `check_render`, handle `ScreenIndex::System` separately:
upload directly from `self.image` via `self.image.borrow()` rather than going
through the pool/LuaImg path.

---

### 3 — Fullscreen quad draw calls use 4 instances instead of 1 **(Visual/Correctness)**

**Symptom**: Sky, GUI, and post-process quads are drawn 4 times.

**Cause**: All three fullscreen-quad passes call `draw(0..4, 0..4)` — 4
vertices × **4 instances** — where only 1 instance is needed.  With
`BlendState::ALPHA_BLENDING`, drawing 4 identical layers accumulates alpha,
making semi-transparent Lua artwork appear significantly more opaque than
intended. For a fully transparent texture (all `a = 0`) the effect is zero, so
empty textures still show the clear colour correctly — but once content is
drawn, the colour will be wrong.

**Fix**: Change to `draw(0..4, 0..1)` for sky, GUI, and post-process draws.

---

### 4 — Secondary / Trinary layers always mirror Primary **(Design Gap)**

**Cause**: In `check_render`, `ScreenIndex::Primary`, `Secondary`, `Trinary`,
and `System` all resolve to `pool.gui.as_ref()` (the same primary LuaImg).
Secondary and Trinary GPU textures receive the same content as Primary.

`BundleMutations` has no fields for secondary/trinary, so there is no
per-frame dirty signal for those layers.

**Fix (future)**: Extend `BundleMutations` with secondary/trinary flags and
expose separate `LuaImg` globals in the Lua VM (or index the pool by layer
index). For now this is noted as a known limitation.

---

### 5 — pool.gui_dirty / sky_dirty optimisation path is incomplete **(Future Work)**

The intended design is:
1. Lua draws to the `gui` / `sky` `LuaImg` global.
2. The Lua loop checks `img.dirty`, sets `shared.gui_dirty = true`, and sends
   `mutations.gui = true`.
3. `check_render` reads `pool.gui_dirty` to decide whether to re-upload.

The code for step 2 is commented out in `lua_define.rs` because it used the
silt-lua API (`apply_userdata_mut`) which needed to be adapted from the old
mlua version. Re-implementing this would restore per-frame dirty culling and
reduce unnecessary GPU texture uploads.

**Short-term fix**: Remove the `mutations.gui/sky = false` overrides so every
Lua frame triggers a texture upload (fine for correctness, slightly wasteful).

**Longer-term**: Uncomment and adapt the dirty-check block using the correct
silt-lua API (`vm.globals.borrow()` / `apply_userdata_mut`) and restore
`pool.gui_dirty` as the gate in `check_render`.

---

### 6 — wgpu 24 surface format was hardcoded **(Fixed in previous commit)**

`Bgra8UnormSrgb` was hardcoded for the surface and intermediate render texture.
wgpu 24 requires querying `surface.get_capabilities()`. This was fixed in
commit `5dcad1d`.

---

### 7 — World mesh not visible without game content **(Expected behaviour)**

With the console/empty app there are no chunks or entities loaded, so the world
render pass produces no geometry. When a game is loaded, `check_ents()` is
called on every `LoopComplete` and should populate `instance_buffers`. This
path is expected to work once the Lua loop correctly signals `LoopComplete`.

---

### 8 — Web canvas resize feedback loop → texture-size crash **(Mitigated; responsive sizing still TODO)**

**Symptom**: On some page loads the canvas grows every frame until it fills the
page, then the module aborts with `Texture size ... exceeded maximum texture
size` (e.g. requesting 10240×8768 against an 8192 device limit).

**Cause**: The canvas had no CSS size, so it *displayed* at its backing-buffer
pixel size. winit sizes the backing buffer from the client rect × devicePixelRatio,
so on a HiDPI display: buffer = client × DPR → (no CSS) display grows to buffer
px → client rect grows → buffer grows again. A runaway that multiplies by DPR
each observation until it passes the GPU's `max_texture_dimension_2d`.

**Mitigation (committed)**:
- `attach_canvas_to_dom` pins the canvas CSS display size (640×548), decoupling
  display from the backing buffer so the loop can't start.
- `Gfx::set_config_size` clamps width/height to `device.limits().max_texture_dimension_2d`
  as a safety net, so any future resize path can't crash the module.

**Still TODO**: The canvas now renders at a *fixed* CSS size. Making it responsive
to the `<petrichor-64>` host container needs a ResizeObserver-driven path that
reads the container's size (not the canvas's own) and sets the buffer from that,
so display never feeds back into buffer. Until then, hosts resize via CSS on
`#petrichor64-root` at their own risk.

---

## Retro Lighting (planned)

Goal: give the 3D pass simple, cheap, era-appropriate lighting (think N64 / PS1 /
Quake-lite) — **no shadow maps, no light decals, no per-pixel light loops**. A
single "sun" plus ambient, and a couple of stylised extras.

**Most of the scaffolding already exists** and is just switched off:
- `Vertex` carries a normal (`_normal: [i8; 4]`, `model.rs`), and `shader.wgsl`
  already passes `world_normal` + `world_position` to `fs_main`.
- `fs_main` even computes `let diff = max(dot(norm, light_dir), .1)` — but line
  ~157 sets `diffuse = light_color` with the `diff *` **commented out**, so
  nothing is actually shaded. The light is also a hardcoded point light orbiting
  on `in.time`.

### Phase L0 — directional sun + ambient (the "turn it on" step)
- Add a light to `GlobalUniforms` (`gfx.rs`): `light_dir: [f32;4]`,
  `light_color: [f32;4]`, `ambient: [f32;4]` (there's a spare slot; the old
  `num_lights` field is already stubbed). Feed it into the `Globals` block in
  `shader.wgsl`.
- Replace the orbiting point light with a fixed directional light and actually
  apply it: `let shade = ambient + max(dot(norm, -light_dir), 0.0) * light_color;`
  then `f_color.rgb *= shade`.
- Expose it to Lua: a `light{ dir = {..}, color = {..}, ambient = {..} }` native
  (mirrors the `cam` native → a `MainCommmand`/`VmToHost` like `Cam`).

### Phase L1 — pick the retro shading model
- **Gouraud (per-vertex)**: move the diffuse term into `vs_main` and interpolate
  it — the authentic PS1/N64 look (cheap, slightly wobbly). Recommended default.
- **Flat**: one normal per face → faceted Quake-lite look (needs face normals or
  `@interpolate(flat)`).
- Keep the current per-fragment path available as the "smooth" option.

### Phase L2 — stylised extras (still no shadow maps)
- **Distance fog**: blend `f_color.rgb` toward a fog colour by depth. The alpha
  fade on `specs.w` (`fs_main` ~163) is the same idea — extend it to colour.
  Very PS1/N64, and hides the far clip.
- **Hemisphere ambient**: tint ambient by `normal.z` (sky colour above, ground
  colour below) for free directionality without a second light.
- **Banded/quantised diffuse**: `floor(diff * n) / n` for a stepped, cel/retro
  ramp — pairs well with the palette look.
- **Vertex-colour tint / baked AO**: the instance already carries a `color`
  attribute; multiply it in so tiles/entities can bake in cheap occlusion.

Constraints to hold the retro line: exactly one directional light (no loops),
lighting stays in the existing single forward pass, and it degrades to
"fullbright" (ambient = 1) so unlit apps look unchanged.

---

## Patches Applied

| File | Change |
|------|--------|
| `src/lua_define.rs` | Remove `mutations.gui = false; mutations.sky = false;` so BundleMutations defaults (`true`) stand |
| `src/gui.rs` | Fix `ScreenLayer::check_render`: upload System from `self.image`; upload Primary/Sky from LuaImg without `pool.gui_dirty` gate; reset dirty only after successful upload |
| `src/render.rs` | Change `draw(0..4, 0..4)` → `draw(0..4, 0..1)` for sky, GUI, and post passes |

---

## Sound System

Native-only optional feature (`--features audio`, pulls `cpal`). One cpal output
stream owns all synth state on the audio thread; the engine sends one-way
`SoundCommand`s over `core.singer` (`mpsc`). The stream outlives Lua reloads, so
`SoundCommand::Reset` (sent from `async_load_app` every load) wipes instruments,
samples, the loaded-file bank, and all voices.

**Design constraints (from the engine owner):**
- Instruments/samples stay **integer-indexed** for a fast mixer hot path — never
  string-keyed. Names exist only in load/bind staging.
- Don't add Lua commands just to associate a sound file with an id — reuse
  `smpl` (string arg binds a loaded file; table arg is raw PCM).
- Load-everything-at-boot is fine for now; **asset unloading is a deferred
  concern** (see below).

### Phases

| Phase | Status | What |
|-------|--------|------|
| 0 | ✅ done | repair scramble/cutting; per-voice wrapped phase + attack/release envelope |
| 1 | ✅ done | polyphony: 16 channels, voice allocation (`pick_channel`), `chord`/`song` |
| 2a | ✅ done | waveforms beyond square/tri: `sine`/`saw`/`pulse`/`noise`/additive (`WaveType`, `osc`) |
| 2b | ✅ done | retro PCM sampling: `smpl` + `voice_out` pitch-resample, `normalize_pcm` loudness-match |
| A (loading) | ✅ done (E2E verified) | load `sounds/*.ogg` (pure-Rust `lewton`) → name bank → `smpl(id,'name')` bind; `Reset` clears bank; `Arc`-shared buffers. Verified: oggify output round-trips through lewton |
| B (oggify) | ✅ done | `tools/oggify` workspace crate: `symphonia` decode (mp3+wav) → `vorbis_rs` encode → `.ogg`; walk a dir, confirm-before-delete originals (default no). Repo is now a workspace (`default-members=["."]` keeps the engine build tool-free); the Vorbis-encoder C dep stays out of the engine + wasm builds |
| 3 | deferred | shared sound VM |
| 4 | deferred | MIDI input (feature-gated) |

### Remaining / gaps
- **Packer bundles `sounds/`** ✅ — `collect_packable_sources` writes `.ogg` files
  (raw wav/mp3 skipped: the engine can't decode them, convert with oggify first);
  verified a packed `.game.png` carries the sounds. Both directory and packed
  games now load sounds.
- **Sample-rate/pitch correctness** ✅ — `Sample.rate_ratio` (= source rate /
  device rate, source read from each ogg header by lewton) makes a note at
  `base_freq` play at the recorded speed regardless of device rate. Raw Lua PCM
  has no source rate so it's 1.0 (device-rate, unchanged). Device rate is a
  load-time constant; nothing is user-passed.
- **Silt `to_vec` hash-order bug** (unfixed upstream): read PCM/chord/song arrays
  by `getn(i)` index loop, never `Vec<T>` `FromLua`.
- **Sample end-click** (minor, theoretical): a sample that doesn't decay to ~0 at
  its buffer end jumps to 0 in one sample when it stops. tone.ogg/amens decay so
  it's inaudible; add a short end-fade if a non-decaying sample ever clicks.

### Deferred: asset unloading (long-term)
The load-everything-into-memory-at-boot model risks large memory footprints for
big games. Not addressed now. This design leans the right way: the sound name
bank is a natural unload hook (drop-by-name / free a slot) and `Arc` buffers
avoid duplication. A full solution (refcounted unload across textures, models,
and sounds) is a separate engine effort for when a game actually needs it.

## Web Audio (AudioWorklet) and the wasm-ultra ladder

The browser plays sound through the **AudioWorklet**: the synth (`petrichor-synth`,
compiled to its own small wasm module) runs *inside* the browser's audio rendering
thread, filling 128-frame blocks (~2.7 ms at 48 kHz). `synth/src/webout.rs` is the
engine's side; `web/synth-worklet.js` is the processor.

`WebAudioOut` (in `synth/src/sound.rs`) is the fallback: it generates audio on the
**main** thread and schedules ~90 ms ahead. Glitch-free but far too laggy to play
music with — it exists so a browser that can't run a worklet still makes sound.
Behind the `web-fallback` feature, listed in `web/index.html` rather than folded
into `wasm` (cargo features only ever *add*, so inside `wasm` it could never be
left out). Measured cost of keeping it: **25 KB raw / 8 KB gzipped**. Keep it on.

### Startup order is the whole game here

Four separate silent-audio bugs came out of this path; all four presented as a
clean console and no sound. What the current design encodes:

1. **A processor is constructed on the audio rendering thread, which a *suspended*
   AudioContext never starts.** So the node isn't created until `pump` sees the
   context actually `Running` (i.e. after a user gesture).
2. **The wasm handover is a handshake, not a post.** The processor sends `hello`
   from its constructor; only then does the main thread send the wasm. Posting at
   node-creation time raced construction and the message was silently dropped.
3. **Send the wasm as raw bytes, never a compiled `WebAssembly.Module`.** Chrome
   refuses to *deserialize* a module inside an `AudioWorkletGlobalScope` (cloning
   one is only defined within an agent cluster; the audio thread is its own). It
   fails on arrival — `postMessage` returns Ok. Deserialization is also
   all-or-nothing per message, so a module and bytes in one envelope die together.
   The worklet sync-compiles the bytes (~289 KB, a few ms, off the main thread
   where the 4 KB sync-compile limit doesn't apply).
4. **Hold commands until the synth confirms it exists.** A processor with no synth
   discards them, and the ones sent at boot are the `instr`/`smpl` definitions — so
   forwarding them into a dead worklet left the fallback with no instruments and
   every note came out as a default beep.

Every failure mode here is now loud: `post_message` results are checked, both ports
carry `onmessageerror` (the event that fires when a message *arrives* but won't
deserialize — the only signal for #3), the worklet acks the wasm with a byte count,
and a worklet that starts but never reports a live synth falls back after ~4 s.

### Command transport

Commands reach the worklet as MessagePack (`encode_command`/`decode_command`, public
on the synth crate so every sender agrees on the format — a command can carry a
decoded ogg, which as a JS array would be one boxed number per sample).

The **VM worker owns a private `MessagePort`** to the worklet, transferred to it by
the main thread once the synth is live, so a note goes from Lua straight to the
audio thread. Two frames of latency came off this:

- one was an **ordering accident** — `pump()` ran at the top of `about_to_wait`, but
  the worker's notes are applied ~120 lines below it, so every note missed the
  forward and waited a frame. `pump()` now runs after the apply loop.
- the other was the **main-thread hop** itself, which the lane removes.

Routing is settled once and **never switches mid-stream**: until the worker is told
which route applies it *buffers*. Switching would reorder — commands in flight to
the main thread would arrive after ones later sent down the lane, and `instr`/`smpl`
landing after the notes that use them is bug #4 again. So the main thread either
transfers the lane or says "no lane is coming" (that message matters: a browser
without AudioWorklet has only the main-thread route, and a worker buffering forever
would be silent). Buffering costs nothing audible — the lane can't open before the
first gesture, and the context is suspended until then.

### Latency budget, and a warning about measuring it

| stage | native | web |
|-------|--------|-----|
| key → Lua (one 60 fps frame) | ~16 ms | ~16 ms |
| `note()` → audio thread | <1 ms (mpsc) | ~0 ms (lane) |
| device block | ~10 ms (CoreAudio) | 2.7 ms (worklet) |
| **engine total** | **~27 ms** | **~19 ms** |

**Bluetooth output adds 100–200 ms** and sits downstream of both, so it swamps
everything above and makes native and web feel identical. Judge latency on wired
output only. If Petrichor is to be usable for performance, the docs should say so
the way every DAW does.

### mpsc vs. a shared-memory ring (the actual trade-off)

They aren't competitors. `mpsc` is an **ownership-transfer channel** (arbitrary Rust
values, allocation allowed); a SAB ring is a **byte pipe** (fixed-size records, no
allocation). Latency isn't the difference — native's mpsc is already excellent, and
the mixer drains it *per sample*, so a native note is sample-accurate to within one
buffer. The ring's value is narrow: crossing a thread boundary on wasm without
postMessage + serde.

What *did* matter was allocation. `Note` carried two `Vec<Consonant>`, was taken by
value on the audio thread and dropped there — `free()` in the audio callback, the
one place that must never wait on the allocator. Fixed by `vocaloid::Cluster`
(4 bursts inline + a length, `Copy`, truncating). Note that plain notes were always
safe: an empty `Vec` doesn't allocate. Only `sing` tripped it.

Still allocating on the audio thread, deliberately: `Chain(Vec<Note>)` (a song is
queued rarely), and `Reset`/`LoadSample` (load time, where a glitch is invisible).
These are the natural contents of a "cold lane" if the transport is ever split.

### The ladder (in value order)

1. ~~Port transfer~~ ✅ done — biggest win, needed no shared memory.
2. **Sample-accurate scheduling.** A *command-schema* property, not a transport one:
   the native mixer already drains per sample, so adding a timestamp field buys
   sample-accurate `arp`/`song` on native today with mpsc untouched. Do this before
   any ring work — it's what makes the clock/`arp` feature feel right.
3. **SAB command ring** (needs `wasm-ultra`). Worker writes fixed-size POD records;
   the worklet reads them in `process()` via `Atomics.load` on the write cursor. No
   serialization, no postMessage. Never `Atomics.wait` on the audio thread —
   polling per render quantum is the correct pattern and costs nothing. Variable
   payloads stay off the ring (`LoadSample` carries 345k floats): keep them on
   postMessage, or put PCM in a separate SAB arena and pass `(offset, len)`. The
   commands are already integer-keyed, which fits.
   **Synergy:** wasm-ultra's shared entity buffer wants the same SPSC-ring
   primitive. Build it once, use it for both.
4. **Shared PCM arena** — kills the duplicated ~4 MB heap, makes `smpl` zero-copy.
   The hard one: `+atomics,+bulk-memory --shared-memory` means nightly and
   `-Z build-std`, and two wasm *instances* can't share `Arc<Vec<f32>>` — it needs a
   hand-managed arena, not Rust-level sharing. Lowest priority; samples load once.
5. **Mic input ring** — same primitive, zero-copy capture.

**wasm-ultra is currently 0 lines of code** (`grep -rn 'feature = "wasm-ultra"' src/`
returns nothing) — the feature is declared and awaiting an implementation. The
worklet needs *nothing* from it: a separate wasm module with its own linear memory,
unaffected by COOP/COEP. Ultra is an upgrade to the transport, not a prerequisite.

## Mobile: Android now, iOS anticipated

### Where it stands

The engine **type-checks for `aarch64-linux-android`** (`cargo check --target
aarch64-linux-android`), touch input works through the existing mouse API, and
`android_main` exists. It has **not been built into an APK or run on a device** —
that needs an Android SDK + NDK, which the dev machine doesn't have (see below).

### `desktop` is not the same as `not(wasm)`

The assumption that "native" implies clipboard, native dialogs and a terminal is
what broke first: `native-dialog` has no Android backend at all. So build.rs now
emits a **`desktop`** cfg (macOS/Windows/Linux/BSD), and the desktop-only crates
(`clipboard`, `native-dialog`, `crossterm`, `midir`) moved to a Cargo target table
gated on the equivalent longhand predicate.

The duplication is forced, not sloppy: a build-script cfg **cannot** drive
dependency resolution, so the condition exists in both places and they must be kept
in step. build.rs says so at the point of definition.

`OS` (visible to Lua) gained `"droid"` and `"ios"`, plus an `"other"` fallback so an
unforeseen platform fails at runtime with an odd name rather than refusing to
compile.

### Touch → `mus()`

Folded into the mouse so every existing game works untouched: the **primary** finger
is the cursor, contact is a left click. Primary means *the first finger down that is
still down*, tracked by winit's touch id — so a second finger landing and lifting
mid-drag doesn't hijack the cursor or release the button, which is what id-less
handling gets wrong. Deltas are accumulated per frame in pixels (touch has no
`DeviceEvent::MouseMotion`), and a tap produces no delta on first contact so it
doesn't read as a flick. Position stays where the finger lifted, like a mouse that
stopped moving.

Not gated to mobile: winit reports touch identically on Android, iOS and desktop
touchscreens, so this also makes a Surface or touch laptop work. Multi-touch
gestures should get their own Lua command rather than being smuggled through `mus`.

### Remaining before it runs a game on a device

1. **Where the game comes from.** Desktop takes a path; web fetches
   `/game.game.png` or falls back to an embedded bundle. An APK has neither — the
   game wants reading out of APK assets via `AndroidApp::asset_manager()`. The
   embedded-bundle path is wasm-only today because it goes through `fetch`.
2. ~~**Surface lifecycle.**~~ ✅ fixed. Android destroys the native window whenever the
   app leaves the foreground (a screen lock is enough) and supplies a new one on
   return; `resumed` had an early `return` when a window already existed, so the
   surface kept pointing at the dead one and the app came back black — exactly as
   predicted. `Gfx` now keeps the `wgpu::Instance` so `recreate_surface()` can rebuild
   just the surface, keeping the device, pipelines and running game; `suspended()`
   stops drawing until then. Verified by sleep/wake and by minimising.
3. **APK packaging.** The `.so` is done (`just android` / `just android release`)
   but nothing wraps it into an installable APK yet. Options: `cargo-apk`
   (simplest for `native-activity`, but unmaintained and untested against NDK 30 /
   build-tools 36), `cargo-ndk` + a Gradle project (most control, most setup), or
   `xbuild`. Packaging needs an `AndroidManifest.xml` whose
   `android.app.lib_name` is `petrichor64` (matching the emitted
   `libpetrichor64.so`) and `minSdkVersion` 26 — see below.
4. **Audio is unverified.** It links, but nothing has produced a sound. Expect this
   to need attention — mobile audio wants larger buffers than desktop.

### minSdk is 26, and audio is why

The first link attempt (API 24) failed with `ld.lld: error: unable to find library
-laaudio`. cpal's Android backend links **AAudio**, which only exists from Android
8.0 (API 26). So the audio feature sets the platform floor; if audio is ever made
optional on Android, 24 becomes reachable again. `android_api` in the justfile is
the single place this is set.

### Verified on hardware (Galaxy Z Fold 5, Android 16)

The APK installs, launches, loads the game baked in with `include_auto`, decodes its
oggs and holds a steady **60 fps**. Two startup aborts had to be fixed to get there —
a hardcoded surface `alpha_mode` and gilrs's missing Android backend, both in
`5bbf327`.

**Touch works and is exact.** A tap at (400, 1200) on a 904x2316 screen reports
`x=0.4425, y=0.5181` — 400/904 and 1200/2316 to four decimals. Press/release edges
fire correctly.

**Unprojection is aspect-correct**, which is the thing a phone was most likely to
break. Off-centre ray deflection per unit of screen space came out 0.315 horizontal
vs 0.809 vertical — a ratio of 2.57 against the screen's own aspect of 2316/904 =
2.56. And visually, a cube placed at `mus().v * 12` lands centred on the crosshair to
within a pixel or two.

`test/touch` is the app that proves it, kept as a smoke test. Three things it
documents by example, each of which looked like an engine bug first: `m1` is a
boolean (not 0/1); `fill()` on the gui layer is *opaque* and hides the 3D scene, so
use `clr()`; and the cube mesh is corner-anchored and untextured by default, so it
needs `tex`/`offset` to show up where you expect.

Sleep/wake and minimise/restore both come back drawing correctly (`suspended —
surface released` / `surface rebuilt after resume` in logcat). The inner (unfolded)
display is 1812x2176 and the app survives being moved to it — the resume path takes
its size from the window rather than the stale config, so a fold or rotation
re-derives the render targets.

### Verified so far

- `libpetrichor64.so` **links** for `aarch64-linux-android`: 300 MB debug,
  **11 MB release**.
- Both entry symbols are exported and the justfile *checks* them rather than
  assuming, because a missing one kills the app at startup with no useful message:
  `android_main` (ours) and `ANativeActivity_onCreate` (android-activity's
  native-activity backend, which is what `android.app.lib_name` resolves).
- `just check-android` type-checks without needing the NDK at all (checking doesn't
  link), so the target can't rot unnoticed.
- The toolchain is discovered, never hardcoded: `$ANDROID_HOME` or the Android
  Studio default, newest NDK under it. `just android-env` prints the shell exports
  worth having (`adb`, `emulator`, `sdkmanager`).

### For iOS later

The `desktop` cfg and the touch handling are already iOS-shaped — iOS is excluded
from the desktop-only deps and reports touch the same way, so it should reach the
same "type-checks" state cheaply. What differs: entry point (winit has an iOS
`EventLoop` path, no `android_main` equivalent), assets come from the app bundle
rather than an AssetManager, audio is CoreAudio via cpal (already supported), and
signing/provisioning is a whole separate problem. The `midi` feature could actually
work there (CoreMIDI), unlike Android.

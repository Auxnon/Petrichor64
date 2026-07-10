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

## Patches Applied

| File | Change |
|------|--------|
| `src/lua_define.rs` | Remove `mutations.gui = false; mutations.sky = false;` so BundleMutations defaults (`true`) stand |
| `src/gui.rs` | Fix `ScreenLayer::check_render`: upload System from `self.image`; upload Primary/Sky from LuaImg without `pool.gui_dirty` gate; reset dirty only after successful upload |
| `src/render.rs` | Change `draw(0..4, 0..4)` → `draw(0..4, 0..1)` for sky, GUI, and post passes |

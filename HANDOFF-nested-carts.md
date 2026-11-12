# Handoff: a cart running inside another cart

sosh wants to run a Petrichor64 cart shared in chat as its own, sandboxed instance,
in a box in the chat. This note covers what the engine has for that and what's
missing. All references are against `main` at `76c0f91`.

## Where sosh is now

- **Web:** done without engine changes. The cart runs in a sandboxed `<iframe>`
  over the chat, holding a second copy of the web engine on its own page
  (`/play/<key>`). That page's Content-Security-Policy lets it fetch the engine
  and that one cart and connect nowhere. It gets full 3D and sound, and has no
  access to the game's state or the network. (sosh: `client/web/bridge.js`,
  `play.js`, `server/lib/sosh_web/controllers/play_controller.ex`.)
- **Android:** the same page, in a web view of its own inside the chat.
- **Native desktop:** not possible yet. The chat says where to play it in a browser.

The web route works because a browser can host two engines side by side, each in
its own document. Native has one window, one GPU context and one `Core`, so the
cart has to run inside it as a child bundle. That's the work below.

## What exists

- **Child bundles.** `sub`/`over` (command.rs:3171, 3184) send
  `MainCommmand::Subload(path, bool)`, which goes to `load_app(.., Some((id, is_overlay)))`
  (lib.rs:3802).
  - Each bundle gets its own silt VM on its own thread (lua_define.rs:245).
  - `make_bundle` (bundle.rs:256-296) looks inverted: `sub` passes `false`, which
    makes the new bundle list the *caller* as its child.
- **Loading from bytes.** `load_app` already takes `payload: Option<Vec<u8>>`
  (command.rs:4033, 4241-4260), which goes to `asset::unpack(file: Vec<u8>)`. That is
  how `INCLUDE_AUTO` loads. Lua can only pass a path, though, and there's no Lua
  base64 decode.
- **Input** goes to `input_owner()` (bundle.rs:302); every other bundle gets
  `ControlState::default()` (`call_loop`, bundle.rs:189-245).
- **Per-bundle state:** entities (filtered by bundle), gui/sky rasters, the VM
  and its `conn` sockets.

## What leaks between bundles today

- **Textures:** one atlas with one namespace. `gimg`/`tex` look up by name only
  (lib.rs:3243-3260, texture.rs:559), so a child can read or overwrite the host's
  textures.
- **Files:** `io.get`/`io.set` resolve against `get_main_bundle()`'s directory
  (lib.rs:3568), which is the host's files. `io.copy` reaches the clipboard.
- **Camera:** a non-overlay `cam` writes the global camera (lib.rs:3109-3119).
- **Global settings:** `lum`, `fog`, `chip`/`mon`, screen effects, `attr`,
  `quit` and `lock` affect the whole engine.
- **Loading a child resets the host's sound and settings:** it calls
  `clean_app_attrs()` and `SoundCommand::Reset` (command.rs:4197-4209).
- **Shared engine-wide:** one synth for everyone, and the model manager.
  `soft_reset` resets both for everyone.

## A gui-only nested cart on native, smallest first

1. **Small:** `sub_bytes(b64, w, h)` from Lua, or `sub` taking a byte string,
   plus a base64 decode.
2. **Medium:** a `sandboxed` flag beside `overlay`, built into `init_lua_sys`
   "by construction", the way `app.*` is kept out of games:
   - no `io`, `conn`, `quit`, `sub`/`over`, `attr`, `chip`/`mon`, `gimg` of host
     names, `cam`/`lum`/`fog`;
   - the main thread drops commands from a sandboxed id that would touch globals;
   - loading a sandboxed child skips the attr and sound resets.
3. **Medium:** a texture and model namespace per bundle, so a child's `tex("x")`
   is its own x.
4. **Small-medium:** route input per bundle by focus: keys only when the child is
   focused, mouse remapped into the rect the host gave it.
5. **Small:** keep the child off the screen's gui layers. Give the host its raster
   as an image it can draw, e.g. a texture the host draws with `im:img`.
6. **Large (3D in the box):** a second camera uniform and bind group, rendered with
   a viewport/scissor into an offscreen target, then blitted into the rect.
   `Bundle.cam` is already recorded but unused (PLAN.md:707-720).

Steps 1-5 give carts that draw with `gui` (most small ones). Step 6 is the
expensive part.

## On the web, inside one engine (if ever wanted)

Today `sub` is ignored on the web:

- the worker's catcher drops everything but Spawn and LoopComplete (worker.rs
  ~365);
- there's one VM per worker, as bundle 0 (lib.rs:1178);
- overlays are native-only.

A nested cart would need:

- a second `WorkerHandle`, which is a whole second wasm instance and its memory;
- `Init{bundle_id}` generalised;
- the cart split (`load_wasm_bundle`) run on bytes;
- input forwarded to it;
- its `Raster` messages composited into the rect (full frames copied each time;
  there's no SharedArrayBuffer in the drop-in build).

The iframe route avoids all of this and gets full 3D, so it's the better route
for the web.

## Fixed on this branch (`fix/web-attr`)

- **`attr` on the web did nothing.** `VmToHost::Globals` was a TODO in
  `apply_vm_message`, so `gui_px`, `modernize`, `title` and the rest were dropped,
  and the web gui stayed 320x240-ish at any window size. It now goes through the
  same `apply_globals` as native.
- **`title` on the web** set the canvas `alt` through winit, which borrows a canvas
  its event loop may hold ("RefCell already borrowed"). It now sets
  `document.title`.

# Handoff: Petrichor64 as a library a host app embeds

Line references are against `main` at `9c69fb2`.

## Why

sosh (a chat-and-world social app) is moving its desktop and web clients out of
a Petrichor cart and into ordinary apps that **host** the engine. The model is
`~/projects/aqua`:

- Its engine is a library that never opens a window.
- The desktop is winit + egui on one wgpu device. The engine draws, then egui
  draws on top in the same encoder (`aqua/desktop/src/main.rs:276`,
  `present_with`).
- Android is Compose with the engine in a `SurfaceView`.
- The web is a JS UI over the wasm engine.

So Petrichor must be able to run as an **instance** inside someone else's
window:

- on the host's wgpu device;
- drawing into a texture the host shows wherever it likes;
- with input handed to it by the host;
- asking the host (not the OS) for window-ish things.

A host may run **several instances at once**: the world, plus a cart shared in
chat. Each instance runs **one cart**. The overlay/child-bundle system (`sub`,
`over`) is out of scope; leave it working as it is for `chor`, but nothing here
needs it. `HANDOFF-nested-carts.md` is superseded by this.

**Done means** all of these hold:

- `chor` behaves exactly as today, because it becomes one host of the new API.
- A small example opens one window and runs two carts side by side, each in its
  own rect, with input going to the focused one.
- Two `<petrichor-64>` elements on one web page run two different carts.

## Where the engine is tied to its window today

| What | Where | Why it matters |
|---|---|---|
| `Gfx` owns the window and surface | `gfx.rs:23-75` (`win_ref`, `surface`, `config`, `instance`) | it creates its own surface, adapter and device: `Gfx::new` / `GpuLink`, `gfx.rs:180-260` |
| Pipelines built for the surface format | `gfx.rs:287-335` (`surface_caps`, `config.format`) | the post and gui-overlay pipelines must be built for the host's target format instead |
| The frame is presented to the surface | `render.rs:556-637` | the only place that needs a target: the post pass draws to `output.texture`, then `present()`. Everything before it draws into `gfx.post.post_texture` already |
| Window calls from `Gfx` | `gfx.rs:1007` (scale factor), `1078-1105` (fullscreen, `request_inner_size`, `set_title`) | these become requests to the host |
| `Core::new(window)` | `root.rs:172-` | builds `Gfx` from a window, and opens its own audio stream (`sound::init`, `root.rs:203`) |
| The frame driver lives in `App` (winit) | `lib.rs:1317-1423` (pacing at a fixed `FPS`), `1330-1380` (input bits → `update` → `controls_evaluate` → Lua loop), `766-797` (render on `RedrawRequested`) | an external host can't call any of it: it's inside `ApplicationHandler` |
| Input is gathered from winit in `App` | `lib.rs:800-990` (`KeyboardInput`, `CursorMoved`, `Touch`, `MouseInput`, `MouseWheel`, `ModifiersChanged`, `Focused`, `DroppedFile`), `device_event` 1000 | into `App.bits: ControlState` (`types.rs:29`: 256 keys + 11 floats) |
| Window and OS state changes | `state_change_checker` (`lib.rs:325`): cursor grab and visibility, fullscreen, title; `controls_evaluate` takes the `ActiveEventLoop` (quit) | must become requests the host may grant or ignore |
| GPU loss and restore | `gpu_loss.rs:26-31` (process-global `LOST` / `WAITING` / `DEVICE`), `App::restore_gpu` | with a host device, the host owns loss |

### Process-global state that breaks two instances in one process

On the web each instance is its own wasm module copy, so statics there are
per-instance already. Native is one process:

- `skeleton.rs:521` `SKELETONS` (by model name) and `skeleton.rs:425` `CLOCK`,
  advanced by `lib.rs:3935-3942` (`BOOT`, `publish_clock`). Two carts with a
  model of the same name would trample each other. Move both into the instance:
  `ModelManager` / `Core`, passed to that instance's Lua threads.
- `gpu_loss.rs` statics: per device, or the host's job (below).
- Fine as they are:
  - `ws.rs:196` `SOCKETS` is thread-local on the VM thread; check the id
    counter is per-thread or globally unique.
  - `command.rs:5042` `VM_CHIP` / `VM_MONITOR` are per VM thread.
  - `midi.rs` (one device for all).
  - `tileset.rs` `OnceLock`s are constant tables.
  - `snap.rs` is test-only.

## The API to build (a sketch; shape it as the code wants)

```rust
/// What the host lends: its device and queue, and the format it will sample.
pub struct HostGpu {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub format: wgpu::TextureFormat, // e.g. Rgba8UnormSrgb
}

pub struct Options {
    pub cart: Cart,      // Cart::Bytes(Vec<u8>) (a .game.png or zip) | Cart::Dir(PathBuf)
    pub size: (u32, u32), // physical pixels of the rect it's shown in
    pub scale: f32,       // the display's scale factor
    pub policy: Policy,
}

/// What the cart may reach beyond its own instance. Enforced by construction:
/// a Lua global that's off is never registered (`init_lua_sys`,
/// command.rs:680), not checked at call time.
pub struct Policy {
    pub network: bool,           // conn (command.rs:3196)
    pub files: Option<PathBuf>,  // io.get/io.set, rooted here; None = no io (command.rs:3341)
    pub clipboard: bool,         // io.copy
    pub console: bool,           // the backtick console and its commands
    pub load_apps: bool,         // load / sub / over / reset to another cart
    pub audio: bool,
    pub host_messages: bool,     // the host channel below
}

impl Instance {
    pub fn new(gpu: &HostGpu, opts: Options) -> Result<Self, String>; // async on wasm
    pub fn resize(&mut self, size: (u32, u32), scale: f32);
    pub fn input(&mut self, ev: Input);       // keys, pointer, wheel, touch, text, focus lost
    pub fn frame(&mut self, now: Duration);   // runs a tick if one is due (the engine keeps its own 60 Hz)
    pub fn render(&mut self);                 // draws into its own texture and submits
    pub fn texture(&self) -> &wgpu::TextureView; // what the host samples (changes on resize)
    pub fn requests(&mut self) -> Vec<Request>;  // GrabPointer(bool), CursorVisible(bool), Title(String), Fullscreen(bool), Quit, Clipboard(String)...
    pub fn post(&mut self, msg: String);         // host -> cart
    pub fn messages(&mut self) -> Vec<String>;   // cart -> host
}
```

Design notes:

- **The instance owns its output texture**, sized to its rect, with
  `RENDER_ATTACHMENT | TEXTURE_BINDING` usage in the host's format. The host
  shows it. In egui that's `egui_wgpu::Renderer::register_native_texture`,
  re-registered on resize. `render()` submits its own command buffer, as
  `render_loop` already does. That avoids threading the host's encoder through
  the engine, and the GPU orders both submissions on one queue.
- **Input coordinates are the instance's own pixels**: the host subtracts its
  rect. Keys use `winit::keyboard::KeyCode`, re-exported, so there is one
  keymap (`bit_check` in lib.rs). An egui-on-winit host has those codes. The
  web and Android hosts translate theirs.
- **A host channel replaces the web bridge hack.** Lua gets `host.send(str)` and
  `host.recv()` (or a `conn("host:")` that never leaves the process).
  `Instance::post` and `messages` are the other end. sosh's world cart uses it
  to tell the app "this player was clicked" and to hear "say this bubble". It
  replaces sosh's `BroadcastChannel` socket (`sosh/client/web/bridge-worker.js`),
  which was only needed because there was no host.
- **Requests, not actions.** `attr{title=..}`, fullscreen, size, cursor
  grab and quit never touch a window in hosted mode; they queue a `Request`. The
  host decides. sosh ignores fullscreen and quit from a shared cart.
- **GPU loss in hosted mode** is the host's: it sees the device die and builds
  new instances on the new device (carts restart), or calls an
  `Instance::rebuild(&HostGpu)` that keeps Lua state. `rebuild` is what
  `App::restore_gpu` does today. Start with the restart and leave a TODO.
- **Audio.** Each instance opening its own cpal stream works, so keep that for
  now with `policy.audio`. A shared mixer and per-instance volume can come later.

## Order of work

1. **wgpu 29 → 30.**
   - `Cargo.toml:112`, `258`. aqua uses wgpu 30 with egui 0.37 / egui-wgpu
     0.37, and sosh's desktop host will too.
   - Fix the API breaks and run the existing tests and the GPU snapshot tests
     (`snap.rs`, `CHOR_SNAP`).
2. **Split the target out of `Gfx`.**
   - Add `enum Target { Surface { window, surface, config }, Texture { texture, view, size } }`.
   - `GpuLink` gets the device either way: requested from a window as now, or
     handed in.
   - The pipelines take the target's format.
   - `render.rs:556` takes the view from the target: acquire and present only
     for `Surface`.
   - The `Gfx` window calls (`gfx.rs:1007`, `1078-1105`) go through the target:
     the surface target does them, the texture target queues `Request`s.
3. **`Core::new(link, target, options)`** instead of `Core::new(window)`.
4. **Pull the frame driver out of `App` into `Instance`.**
   - Covers input bits, `state_change_checker` (as requests), `update`,
     `controls_evaluate` (without the event loop: quit becomes a request), the
     Lua loop call, `render` and pacing.
   - `App` becomes a thin winit host: one `Instance` on a `Surface` target. It
     translates `WindowEvent`s into `Input` and grants every `Request`.
   - This keeps one code path, so `chor` is the first test of the new API.
5. **Per-instance statics:** `SKELETONS`, `CLOCK`, gpu_loss.
6. **Carts from bytes.**
   - `load_app(core, None, Some(bytes), ..)` exists (command.rs:4033)
     and goes to `asset::unpack`. Expose it as `Cart::Bytes`.
   - It must not touch files: no writing the unpacked cart to disk unless
     `policy.files` allows.
7. **`Policy`, enforced in `init_lua_sys`.**
   - With `files: None`, `io` isn't registered.
   - With `network: false`, `conn` isn't registered.
   - Likewise the clipboard, the console and loading other apps.
   - The main thread also drops `MainCommmand`s an instance's policy forbids,
     in case one arrives anyway.
8. **The host channel** (Lua `host.*`, `Instance::post` / `messages`). On the web
   it's postMessage between the main thread and that instance's worker.
9. **Example:** `examples/two_carts.rs`. winit + wgpu, one window, two
   instances drawn as two quads (or egui, if that's no harder). Click to focus,
   input to the focused one.
10. **Web: more than one engine per page.** Today there is exactly one:
    - `web/petrichor64-element.js` looks up `#petrichor64-root` by id, so a
      second element reuses the first one's root.
    - `import(moduleUrl)` is cached, so the module (and winit's one event loop)
      starts once.
    - The cart is always `/game.game.png` (`lib.rs:1704`); the worker is always
      `/worker.js` (`lib.rs:1103`).

    Wanted:
    - an exported `start(canvas, { cart, worker, policy })`;
    - a fresh module copy per element (a distinct import URL, or `initSync` with
      new memory);
    - `<petrichor-64 cart="..." worker="...">` attributes, no fixed id;
    - the host channel as a JS API on the element (`el.post(str)`, a `message`
      event).

    Done: `start(root, { cart, worker, policy })` (`start_on`/`WebHost` in
    `lib.rs`), a module copy per element (`?i=N`, `web/petrichor64-element.js`,
    which sets `__petrichor64_manual` so `wasm_main` doesn't auto-start), and
    `web/two.html` (packing its two carts is described in it). On an element a
    title request is a `title` event and the gpu events fire on it too;
    `web/index.html` behaves as before.
11. **Later, optional: Android.** A JNI entry that runs an `Instance` on a
    `SurfaceView`'s window, as aqua does (`aqua/app/.../CanvasSurfaceView.kt`,
    `aqua/engine/src/android.rs`). sosh's Android app could then drop its web
    view.

## Checks

- `cargo test`, `cargo check` with default features and `--no-default-features`
  where it built before, and a wasm build (`just web-build`).
- `chor` on a few test carts looks and plays the same, including the snapshot
  tests and resize.
- GPU loss on `chor`: `CHOR_GPU_LOSE=secs` still restores.
- The two-cart example: both animate; keys go only to the focused one; a cart
  with `network: false` gets `nil` for `conn`; one with `files: None` gets
  `nil` for `io`.
- Two `<petrichor-64>` elements with different carts on one page.

## What sosh will build on top (for context, not part of this work)

- Desktop: winit + egui, styled like the Android app. Its chat, servers and
  friends are native UI, and the world is one `Instance` in a rect. A cart shared
  in chat is another `Instance` with `Policy { network: false, files: None,
  clipboard: false, console: false, load_apps: false, .. }`.
- Web: a JS UI with `<petrichor-64>` elements.
- Android: Compose as now.

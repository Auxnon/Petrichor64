# Petrichor64 on the Web (wasm32)

Status of the browser port and the work remaining.

## Done — it compiles

Both Petrichor and the embedded `silt-lua` now build for
`wasm32-unknown-unknown` alongside the native targets:

```sh
cargo build --target wasm32-unknown-unknown          # bin + lib
cargo build --target wasm32-unknown-unknown --lib
```

What that took:

- **Dropped `async_zip`'s `lzma` feature.** Only `Compression::Stored` is used,
  and `lzma` pulled `liblzma-sys` — a C library with no wasm target.
- **Split `tokio`.** The shared dependency now carries only the wasm-safe
  features (`sync`, `macros`, `io-util`, `rt`); `full` (which pulls `mio`/net) is
  added back only for native targets.
- **Target-gated the desktop-only crates.** `clipboard` and `native-dialog`
  moved to a `cfg(not(target_arch = "wasm32"))` dependency table; their call
  sites are gated in `lib.rs` / `controls.rs`. A wasm dependency table adds
  `wasm-bindgen`, `wasm-bindgen-futures`, `js-sys`, `web-sys`, `web-time`,
  `console_error_panic_hook`, and `console_log`.
- **`Instant`** aliases to `web_time::Instant` on wasm (winit's `WaitUntil`
  expects it) and `std::time::Instant` on native.
- **`pack_zip`** (writes a game bundle to disk via `tokio::fs`) gets a wasm stub
  that reports packing as unsupported.
- **silt-lua**: its standalone `#[wasm_bindgen]` JS exports (`run`, `jprintln`,
  `lsp`) were gated on `target_arch` while their `wasm_bindgen` import was gated
  on the `wasm` *feature*, so a wasm build without that feature failed. They are
  now consistently behind `#[cfg(feature = "wasm")]` and the stale bodies were
  repaired. Petrichor embeds silt in-process and does not enable that feature.

## Harness

- `web/index.html` + `Trunk.toml` drive the browser build via
  [trunk](https://trunkrs.dev):

  ```sh
  trunk serve            # dev server + live reload at http://localhost:8080
  trunk build --release  # emits web/dist
  ```

- Web entry: the bin's `main()` calls `start()`. On wasm, `start()`
  (`lib.rs`) installs the panic hook + console logger and uses winit's
  `spawn_app` (not `run_app`) so control returns to JS and frames are driven by
  `requestAnimationFrame`.

## Remaining — making it run

Compiling is not running. Two coupled architectural changes are required before
the browser build does anything useful. Both sit in the `load_app` init path,
so neither can be skipped to get "just wgpu on screen".

### 1. Async initialisation

`App::resumed` currently does `pollster::block_on(Core::new(window))`, and
`Core::new` → `Gfx::new` awaits `request_adapter` / `request_device`. **The
browser forbids blocking the main thread**, and those requests only resolve when
control returns to the JS event loop.

Plan:
- Split `App` into a `Loading` / `Ready` state.
- In `resumed` (wasm): create the window synchronously, then
  `wasm_bindgen_futures::spawn_local` a task that awaits `Core::new(window)` and
  delivers the finished `Core` back to `App` (via a channel polled each frame,
  since the future can't hold `&mut App`).
- Keep the native path on `block_on`.

### 2. Lua VM on a web worker

`LuaCore::start` (`lua_define.rs:240`) and `World` (`world.rs:225`) run on
spawned OS threads and communicate over blocking mpsc, with a `sync_channel(0)`
rendezvous for `func`/`load`/`die`. `std::thread::spawn` *compiles* on wasm but
**panics at runtime**.

The VM runs on a **web worker**, not inline. This preserves the engine's
existing multi-threaded, message-passing shape:

- Each web worker is a *separate wasm instance* with its own linear memory and
  thread, so the silt VM is created and lives entirely inside the worker — it
  never crosses a thread boundary and `!Send`/gc-arena is a non-issue. (This is
  why the inline-VM approach was rejected.)
- Main↔worker communicate via `postMessage`, which maps directly onto today's
  `LuaTalk` (in) / `MainPacket` + `MainCommand` (out) channels. wgpu stays on the
  main thread; the worker only runs Lua and posts back entity / image / draw
  state.
- The seam is exactly `bundle.lua.start(...)` (`bundle.rs`, → `lua_define.rs`)
  and `world.make(...)`. On wasm, `start` spawns a worker instead of a thread;
  the rest of `Core` is unchanged.

Work required:
- Make `LuaTalk` / `MainCommand` (de)serializable across `postMessage` (serde;
  the crate already has serde + optional rmp-serde). Payloads that carry channels
  or GPU handles need message-shaped equivalents.
- The blocking `sync_channel(0)` rendezvous (`func`/`load`/`die`) must become
  async request/response — `postMessage` can't block the main thread.
- A worker entry point (wasm export invoked from the worker's `onmessage`) that
  instantiates a silt VM and runs the dispatch loop one message at a time.
- Chose separate-instance workers over SharedArrayBuffer / wasm-atomics to avoid
  the COOP/COEP header requirements.

### The web component

Ship the main thread as a `<petrichor-64>` custom element (`web/petrichor64-element.js`)
that, on connect, creates the render host (`#petrichor64-root`, which
`attach_canvas_to_dom` targets), loads the wasm module, and — once ready —
auto-spawns the VM worker(s). Drop-in embeddable, no page-level wiring.

### 3. Assets over the network

`std::fs` / `File` (asset, template, file_util) have no meaning in the browser.
Asset loading (`test/basic`, textures, models) must move to `fetch` (async) or
be embedded with `include_bytes!`. The default boot app should be embedded so
the engine has something to run without a round-trip.

## Progress / order

1. ~~Async init (#1)~~ **done** — `App` builds `Core` via `spawn_local`, the frame
   loop installs it, `attach_canvas_to_dom` mounts winit's canvas. wgpu
   initialises in-browser (verify: `trunk serve`).
2. ~~Web component shell~~ **done** — `web/petrichor64-element.js`, `<petrichor-64>`.
3. ~~Serializable protocol~~ **done** — `src/worker_protocol.rs` (`HostToVm` /
   `VmToHost` / `ValueWire`), `LuaEnt: Serialize`. Confirmed silt persists loaded
   fns across `enter()` via `load_fn -> usize` + `call_fn(idx)` (no silt change).

### VM worker (#2) — the core, sequenced

4a. ~~**Split `lua_define.rs::start()`**~~ **done** — `LuaContext` holds the
    persistent non-`'gc` state (compiler, scripts, loggy, main/loop/draw/drop fn
    indices, key/mouse mutexes, VM→host sender) and `handle_lua_talk(m, vm, mc,
    ctx, local_pool, shared)` is the shared per-message dispatcher. Native builds
    `ctx` once inside its `enter` and the loop just calls the handler; wasm will
    build the same `ctx` and re-`enter` per message. Behaviour-preserving
    (headless VM loop verified).
4b. ~~**Abstract the VM→host sink.**~~ **done** — no sink abstraction needed: the
    worker keeps its own local `MainPacket` channel, so native fns send
    `MainCommand` unchanged and the worker drains + translates after each message.
    `command::main_command_to_host(cmd) -> Option<VmToHost>` maps the
    fire-and-forget subset (Cam/Globals/LoopComplete/AsyncError/Spawn; Spawn
    clones its `LuaEnt` out of the `UserDataWrapper`). Blocking + host-internal
    commands return None (read-back phase). `VmToHost` gained `Error(String)`.
4c. ~~**wasm worker glue**~~ **done + VALIDATED IN BROWSER**. `src/worker.rs`
    holds a `WorkerVm` (`Lua` arena + `LuaContext` + `SharedPool` + local
    channels) in a thread-local. `HostToVm::Init` builds it via `build_worker_vm`
    (same setup as `LuaCore::start`: rasters → globals, `init_lua_sys`, `load_fn`
    ×4; worker-local channels; font self-provisioned from the embedded fallback;
    NO InitBack — the worker owns the gui/sky pixels). Each non-Init message
    re-`enter`s the arena, runs `handle_lua_talk`, drains the local `catcher`
    (→ `main_command_to_host` → `VmToHost` posted back) and `loggy_rx` (→ console).
    `web/worker.js` is an ES-module worker importing `/Petrichor64.js`.
    Confirmed: VM builds, `loop()` runs per frame, globals persist across
    re-enters (count hit 30/60), `cout`→loggy→console, `LoopComplete` streams to
    main. Fixes along the way: `start()` no-ops with no `Window`; Load's reply
    receiver must outlive the `enter`.
4d. **Host side.** On wasm, `bundle.lua.start` spawns the worker instead of a
    thread; the `<petrichor-64>` element spawns it on load. Feed `VmToHost` into
    the existing `Core::update` path; send `Loop`/input as `HostToVm`. The worker
    must also receive the bundle's `BundleResources` (letters/rasters/size) over
    postMessage to build its VM.

5. Embed the default boot app (`include_bytes!`) so no fetch is needed to start.
6. `fetch`-based asset loading (#3) → arbitrary games load.

Read-back calls (func/get_img/get_global/read/write/group/die) come after 4:
they become poll/mirror (network-style, `try_recv` idiom) or request/response by
correlation id — none block the worker.

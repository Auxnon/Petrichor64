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

### 2. Inline Lua VM (no threads)

`LuaCore::start` (`lua_define.rs:240`) and `World` (`world.rs:225`) run on
spawned OS threads and communicate over blocking mpsc, with a `sync_channel(0)`
rendezvous for `func`/`load`/`die`. `std::thread::spawn` *compiles* on wasm but
**panics at runtime**, and silt's VM is gc-arena based (`Rc`, `!Send`), so web
workers are not an option.

The VM must run **inline on the main thread**:
- Under `cfg(target_arch = "wasm32")`, don't spawn a thread. Hold the
  `silt` arena in the bundle and drive it by re-entering `lua_instance.enter(|vm,
  mc| …)` once per dispatched `LuaTalk` message instead of looping forever inside
  a single `enter`.
- Loaded function handles (`main_fn`, `loop_fn`, …) are `Gc` pointers valid only
  within an `enter` scope, so they must live in the arena root / globals and be
  re-resolved each frame rather than stored across frames. **This touches
  silt-stable's API** (a way to store/reinvoke named callbacks across `enter`
  calls) and is the larger half of the work.
- Replace the blocking `func`/`call_loop`/`load` channel calls with direct
  synchronous calls on wasm; keep the channel path on native.

### 3. Assets over the network

`std::fs` / `File` (asset, template, file_util) have no meaning in the browser.
Asset loading (`test/basic`, textures, models) must move to `fetch` (async) or
be embedded with `include_bytes!`. The default boot app should be embedded so
the engine has something to run without a round-trip.

## Suggested order

1. Embed the default boot app (`include_bytes!`) so no fetch is needed to start.
2. Async init (#1) → wgpu clears the canvas in-browser. First visible milestone.
3. Inline VM (#2, incl. the silt-stable arch change) → the Lua loop runs.
4. `fetch`-based asset loading (#3) → arbitrary games load.

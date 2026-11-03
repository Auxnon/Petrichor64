# Handoff: the web build never recovers from a lost GPU

For whoever picks this up in petrichor64. It was found from sosh
(`~/projects/sosh`, the game in `client/`), which ships this engine's web build
in `server/priv/static`. `HANDOFF.md` is a separate, general handoff from
another session, so read that too.

**Status (2026-11-03):** the GPU restore below has been built, on branch
`fix/gpu-restore-oom` (`src/gpu_loss.rs`, the `petrichor-gpu-lost` /
`petrichor-gpu-restored` events). Checked from sosh on WebGL2: a lost and
restored context comes back in about 0.1 s, with the world intact and no
reload. The worker GC fix further down is on `main`.

## Symptom

Reported by the owner, in Chrome on Linux: "sometimes if i change windows and
come back the web client is frozen, purple sky". The page doesn't crash. The
world just stops drawing.

## What's known

- **Purple is the engine's own clear colour.** The post pass clears to
  `Color { r: 0.5, g: 0., b: 0.5 }` (`src/render.rs`, the
  `post_texture_view` pass around line 403). A game draws its own sky over
  that (sosh draws a blue gradient into the `sky` raster), so a purple screen
  means none of the bundle's layers are reaching the screen. The VM and the
  page can still be running.
- **A lost WebGL2 context is never recovered.** Reproduced in headless Chrome,
  which has no WebGPU, so wgpu ran on WebGL2. After
  `WEBGL_lose_context.loseContext()`, then `restoreContext()` 3 s later, the
  page kept running at about 61 requestAnimationFrame/s but drew a black screen
  from then on. There was no console error and no recovery. Purple instead of
  black is probably the same failure caught partway: the clear still happens,
  but the textures, pipelines and buffers are gone.
- **Nothing in the engine listens for a loss.**
  - On WebGL2: no `webglcontextlost` / `webglcontextrestored` handling.
  - On WebGPU: no device-lost callback.
  - `device.on_uncaptured_error` (`src/gfx.rs:227`) calls `error_window` and
    `process::exit`, which is the wrong response on the web anyway.
  - `render()` maps `CurrentSurfaceTexture::Lost` / `Outdated` to
    `DrawState::Resize` (`src/render.rs:562`). That reconfigures the surface
    (`src/lib.rs:652`), which does nothing for a dead device or context.
- **Which backend the owner is on is unknown.** `default_backends()`
  (`src/gfx.rs:1215`) picks `BROWSER_WEBGPU | GL` in a secure context that has
  `GPUCanvasContext`, and `localhost` counts as secure. So the owner's Chrome
  may well be on WebGPU, where a loss arrives as `GPUDevice.lost` and never as
  a canvas event. The engine logs `WebGPU unavailable (...), using WebGL2` only
  when it falls back.
- **Not the cause (ruled out from sosh):**
  - A 40 s `Page.setWebLifecycleState frozen` and then `active`.
  - A 60 s stall of the VM worker's loop (a busy-wait inside `loop()`): the
    queued websocket frames drained and the game carried on.
  - The reconnect path.
  - Headless Chrome can't really background a tab (`visibilityState` stays
    `visible`), and there is no Xvfb on this machine, so the real "change
    windows" case was never reproduced. Only the simulated context loss was.

## Repro (WebGL2)

Playwright is `createRequire('/home/makeavoy/projects/ec-frontend/node_modules/')`
(v1.60). Launch Chromium with
`--use-gl=angle --use-angle=swiftshader --enable-unsafe-swiftshader`, load any
web build (sosh's dev server is `http://localhost:4000/`), wait for it to draw,
then:

```js
await page.evaluate(() => {
  const c = document.querySelector('canvas');
  const gl = c.getContext('webgl2'); // the engine's own context, if it's on GL
  window.__lc = gl.getExtension('WEBGL_lose_context');
  window.__lc.loseContext();
});
await page.waitForTimeout(3000);
await page.evaluate(() => window.__lc.restoreContext());
// screenshot: black from here on, frames still ticking
```

On WebGPU, `getContext('webgl2')` on that canvas returns null. A loss there can
be forced with `device.destroy()`, but only from inside the engine, where the
device lives, or by wrapping `GPUAdapter.prototype.requestDevice` before the
engine starts, as sosh's stopgap does.

## What a fix needs

1. **Detect the loss on both backends.**
   - WebGL2: listen for `webglcontextlost` on the canvas (call
     `preventDefault()` so the browser may restore it) and for
     `webglcontextrestored`.
   - WebGPU: set wgpu's device-lost callback (`Device::set_device_lost_callback`
     in recent wgpu; check the version in `Cargo.lock`).
   - Separate the uncaptured-error path from the lost path, and don't exit on
     the web.
2. **Rebuild the GPU side without losing the game.** `Gfx::recreate_surface`
   (`src/gfx.rs:888`, used for Android's resume in `src/lib.rs:416`) already
   does this for the surface alone. A lost device needs more: a new adapter,
   device and queue, the pipelines, and every texture and buffer. That means
   CPU-side copies of the atlas, the gui and sky rasters, models and so on, or
   re-running the asset load. The VM and its state live in the worker and
   needn't be touched; a full re-send of entity state may be needed after.
3. **Or, as a first step, fail loudly.** A `petrichor-gpu-lost` DOM event (or
   a status the game can read) would let a host page or a game recover with a
   reload, instead of each host guessing.

## Stopgap in sosh

`sosh/client/web/recover.js`, injected after `gamepad.js` by sosh's
`client/justfile` `web-engine` recipe and its `Containerfile`:

- It reloads the page on `webglcontextlost` (1.5 s later, if visible), on
  `webglcontextrestored`, or when a hidden tab with a lost context becomes
  visible again.
- For WebGPU, it wraps `GPUAdapter.prototype.requestDevice` and reloads when
  `device.lost` resolves for any reason other than `"destroyed"`.
- It reloads at most once a minute (in `sessionStorage`).
- It's verified on WebGL2 (the repro above now comes back in about 2 s). The
  WebGPU path is untested: there's no WebGPU in headless Chrome here.

Delete it once the engine recovers on its own.

## Also found from sosh, same day

- **The web VM worker never collected garbage (fixed in `src/worker.rs`,
  on `main`).** `dispatch` runs each message through `lua.enter(...)`, and
  silt's `enter` leaves collection to the caller (only `run`/`call`/`cycle`
  call `collect_debt` themselves). So every table a `loop()` made was kept:
  a cart making ~6 KB of short-lived tables a frame grew the worker 5 MB/s,
  and sosh grew it 355 KB/s (~1 GB/hour) until an allocation failed,
  showing up as `unreachable` in the ws `onmessage` closure, or a
  `RangeError` in `passStringToWasm0`'s `setInt32`. The fix is
  `lua.collect();` after that `enter`. Measured by reading
  `memory.buffer.byteLength` from a patched `worker.js`: the same carts now
  stay flat (21.4 MB / 24.4 MB over 3 min).
  - **Native probably has the same gap.** The `LuaContext` doc in
    `src/lua_define.rs` says native runs the whole VM inside one long-lived
    `enter`, and gc_arena can only collect between `enter`s, so native may
    never collect at all. That could be part of the Mac-left-asleep OOM too.
    Not checked: watch `chor`'s RSS with a cart that makes garbage every
    frame.
- **Sosh's main thread sits at 1.48 GB of wasm memory** (an empty cart:
  66 MB). It's flat, not growing, but large for phones. Possibly the CPU
  copies kept for the GPU restore, or chunk meshes. Not looked into.

- **`now()` sometimes doesn't advance between two `loop()` calls on the web.**
  Games that compute `dt = now() - last` get `dt == 0` on some frames. In sosh
  that zeroed the walking speed every few frames, because a frame with no step
  looked like being blocked. sosh now ignores dt 0 frames. Worth a look:
  - On the web the worker's clock is whatever `HostToVm::Loop { clock }`
    carries (`src/worker.rs:255`).
  - The host posts a `Loop` per frame (`src/lib.rs:1133`).
  - The clock is advanced by `publish_clock()` (`src/lib.rs:3788`).
  - If a frame posts a `Loop` without a `publish_clock` in between (or two
    `Loop`s per clock), the VM sees the same time twice.
- **No silt bug in `if a > x and (expr) / v < 0 then`.** sosh briefly blamed
  silt for that, wrongly; the silt agent was asked to look and couldn't
  reproduce it. It was the dt 0 frames above. silt-stable (20cfec2, the
  GitHub `main` the engine builds from) and plain Lua give identical output on
  sosh's movement code. Nothing to add to `SILT-BUGS.md`.
- **The engine console swallows keys.** With it open, the host sends the VM a
  neutral key snapshot (`src/lib.rs:1129`). In automated tests the console
  overlay sometimes showed up on its own after load, so pressed keys did
  nothing. Click-driven tests weren't affected.

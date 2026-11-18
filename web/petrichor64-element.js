// <petrichor-64> — drop-in custom element that hosts a Petrichor64 engine.
// Any number of them can share a page, each running its own cart.
//
// Usage:
//   <script type="module" src="/petrichor64-element.js"></script>
//   <petrichor-64 cart="/a.game.png"></petrichor-64>
//   <petrichor-64 cart="/b.game.png" policy="sandbox"></petrichor-64>
//
// Attributes (read once, when the element is connected):
//   module  URL of the wasm-bindgen JS module (default `/Petrichor64.js`, what
//           trunk emits into web/dist).
//   cart    URL of the .game.png to run (default `/game.game.png`, else the
//           engine's embedded default cart).
//   worker  URL of the VM worker script (default `/worker.js`).
//   policy  what the cart may reach: `open` (default), `sandbox`, or a JSON
//           object of host::Policy fields over `open`, e.g.
//           '{"network":false,"files":null}' (src/host.rs).
//
// JS API:
//   el.post(str)  host -> cart, read by the cart's `host.recv()`. Posts made
//                 before the engine is up are kept and sent once it is.
//   "message"     CustomEvent per cart `host.send(str)`; `detail` is the string.
//   el.hook(key, value)  host -> cart, a keyed event: the engine calls the
//                 cart's `hook(key, value)` with it before its next `loop()`.
//                 Both are strings. Kept until the engine is up, like posts.
//                 False if the key is over 256 bytes or the value over 64 KiB.
//   "hook"        CustomEvent per cart `host.hook(key, value)`; `detail` is
//                 `{ key, value }`.
//   "title"       CustomEvent when the cart asks for a window title (`detail`);
//                 the element leaves document.title alone.
//   "petrichor-gpu-lost" / "petrichor-gpu-restored"  plain Events (also on window).
//   el.engine     the module's `Engine` once started, el.ready a Promise of it.
//
// Each element imports its own copy of the module (the URL gets `?i=N`): the
// engine's statics and winit's one event loop per module mean one engine per
// copy. Each copy then spawns its own VM worker and AudioContext.
//
// The canvas goes in the element itself (light DOM), sized to fill it; size
// the element to size the game (it defaults to 640x548 if nothing does).

// Before any copy of the module loads: tells its auto-start (wasm_main in
// lib.rs) to leave starting to the elements. A page that loads the module
// without this file (web/index.html) starts one engine as before.
globalThis.__petrichor64_manual = true;

let copies = 0;

class Petrichor64Element extends HTMLElement {
  constructor() {
    super();
    this.engine = null;
    this._pending = [];
    this._pendingHooks = [];
    this.ready = new Promise((resolve, reject) => {
      this._resolve = resolve;
      this._reject = reject;
    });
  }

  connectedCallback() {
    if (this._booted) return;
    this._booted = true;
    if (getComputedStyle(this).display === "inline") this.style.display = "block";
    const moduleUrl = this.getAttribute("module") || "/Petrichor64.js";
    this._boot(moduleUrl).catch((err) => {
      console.error("<petrichor-64>: failed to start engine:", err);
      this._reject(err);
    });
  }

  async _boot(moduleUrl) {
    const url = new URL(moduleUrl, document.baseURI);
    url.searchParams.set("i", String(++copies));
    const mod = await import(url.href);
    await (mod.default ?? mod.init)();
    const opts = {};
    for (const k of ["cart", "worker", "policy"]) {
      const v = this.getAttribute(k);
      if (v !== null && v !== "") opts[k] = v;
    }
    this.engine = mod.start(this, opts);
    for (const m of this._pending) this.engine.post(m);
    this._pending.length = 0;
    for (const [k, v] of this._pendingHooks) this.engine.hook(k, v);
    this._pendingHooks.length = 0;
    this._resolve(this.engine);
  }

  post(msg) {
    msg = String(msg);
    if (this.engine) this.engine.post(msg);
    else this._pending.push(msg);
  }

  hook(key, value) {
    key = String(key);
    value = value === undefined || value === null ? "" : String(value);
    if (this.engine) return this.engine.hook(key, value);
    // the engine's caps, checked here too so the answer doesn't wait for it
    const bytes = (s) => new TextEncoder().encode(s).length;
    if (bytes(key) > 256 || bytes(value) > 64 * 1024) return false;
    this._pendingHooks.push([key, value]);
    if (this._pendingHooks.length > 1024) this._pendingHooks.shift();
    return true;
  }
}

if (!customElements.get("petrichor-64")) {
  customElements.define("petrichor-64", Petrichor64Element);
}

export { Petrichor64Element };

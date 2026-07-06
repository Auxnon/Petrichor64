// <petrichor-64> — drop-in custom element that hosts the Petrichor64 engine.
//
// Usage:
//   <script type="module" src="./petrichor64-element.js"></script>
//   <petrichor-64 module="./pkg/petrichor64.js"></petrichor-64>
//
// The `module` attribute points at the wasm-bindgen ES module (build with
// `wasm-bindgen --target web`, or use trunk's emitted JS during dev). On
// connect the element creates the render host that the Rust side mounts
// winit's canvas into (`#petrichor64-root`, see attach_canvas_to_dom in
// lib.rs), then loads and initialises the wasm module.
//
// NOTE: the engine's Lua VM will run on a web worker spawned once the module
// signals "core ready" — that wiring lands with the worker message bridge
// (see WASM.md §2). This element already sets up the host + module load so
// wgpu initialises and the canvas renders.

const ROOT_ID = "petrichor64-root";

class Petrichor64Element extends HTMLElement {
  static get observedAttributes() {
    return ["module"];
  }

  connectedCallback() {
    if (this._booted) return;
    this._booted = true;

    // Light DOM (not shadow): Rust locates the host via
    // document.getElementById, which does not cross shadow boundaries.
    let root = document.getElementById(ROOT_ID);
    if (!root) {
      root = document.createElement("div");
      root.id = ROOT_ID;
      root.style.width = "100%";
      root.style.height = "100%";
      this.appendChild(root);
    }

    const moduleUrl = this.getAttribute("module");
    if (!moduleUrl) {
      console.error(
        "<petrichor-64>: missing `module` attribute (URL of the wasm-bindgen JS module)."
      );
      return;
    }

    this._boot(moduleUrl).catch((err) => {
      console.error("<petrichor-64>: failed to start engine:", err);
    });
  }

  async _boot(moduleUrl) {
    // The wasm-bindgen module's default export initialises the wasm instance;
    // our bin's main() (the module entry) then runs start() → spawn_app.
    const mod = await import(moduleUrl);
    const init = mod.default ?? mod.init;
    if (typeof init === "function") {
      await init();
    }
    // Worker(s) will be spawned here once the message bridge exists.
  }
}

if (!customElements.get("petrichor-64")) {
  customElements.define("petrichor-64", Petrichor64Element);
}

export { Petrichor64Element };

// Petrichor64 VM web worker (WASM.md §4c).
//
// A worker is its own wasm instance with its own memory, so the silt VM lives
// entirely here and never crosses a thread boundary. This harness loads the
// wasm bundle, initialises it, and relays messages: each `HostToVm` from the
// main thread goes to `worker_receive`, and any `VmToHost` it returns is posted
// back.
//
// Trunk emits stable names (filehash=false) so we can import the bundle by a
// fixed path. It's an ES-module worker: spawn with { type: "module" }.
import init, { worker_init, worker_receive } from "/Petrichor64.js";

let ready = false;
const queue = [];

async function boot() {
  await init(); // instantiate this worker's own wasm memory
  worker_init(); // panic hook + console logger
  ready = true;
  self.postMessage({ kind: "worker-ready" });
  // Flush anything that arrived before init finished.
  for (const msg of queue) dispatch(msg);
  queue.length = 0;
}

function dispatch(msg) {
  // worker_receive returns an envelope { msgs: VmToHost[], ents?: Uint8Array }.
  // Post it as ONE message per frame; transfer the entity buffer zero-copy so
  // its backing ArrayBuffer moves instead of being structured-clone copied.
  const env = worker_receive(msg);
  const ents = env.ents;
  const transfer = ents ? [ents.buffer] : [];
  self.postMessage({ kind: "vm-batch", msgs: env.msgs, ents }, transfer);
}

self.onmessage = (e) => {
  if (!ready) {
    queue.push(e.data);
    return;
  }
  dispatch(e.data);
};

boot().catch((err) => {
  console.error("[petrichor worker] boot failed:", err);
});

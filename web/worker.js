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
  const replies = worker_receive(msg);
  if (Array.isArray(replies)) {
    for (const r of replies) self.postMessage({ kind: "vm", payload: r });
  }
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

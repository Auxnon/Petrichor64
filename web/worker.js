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

// ---------------------------------------------------------------------------
// Sound commands take one of two routes, and never both at once.
//
//   lane  — a MessagePort straight to the AudioWorklet's synth, transferred to us
//           by the main thread once the synth is live. A note goes from Lua to the
//           audio thread directly.
//   host  — back to the main thread in the frame envelope, which forwards them.
//           Slower by up to two frames, and the only route that exists when the
//           browser can't run an AudioWorklet and the main-thread scheduler is
//           doing the playing.
//
// Until the main thread says which one applies, commands are *buffered* rather
// than sent either way. Switching routes mid-stream would reorder them: commands
// already in flight to the main thread would arrive at the synth after ones we
// later sent down the lane, and `instr`/`smpl` definitions landing after the notes
// that use them is exactly how you get a synth full of default beeps. Buffering
// costs nothing audible — the lane can only open after a user gesture, and until
// that gesture the browser keeps the AudioContext suspended and silent anyway.
let soundPort = null;
let laneExpected = true;
let soundQueue = [];
// Cap so a lane that never arrives (a page nobody ever clicks) can't grow this
// without bound. Oldest go first, as with any dropped queue.
const SOUND_QUEUE_MAX = 4096;

function queueSounds(sounds) {
  for (const bytes of sounds) soundQueue.push(bytes);
  if (soundQueue.length > SOUND_QUEUE_MAX) {
    soundQueue = soundQueue.slice(-SOUND_QUEUE_MAX);
  }
}

function postToLane(sounds) {
  for (const bytes of sounds) soundPort.postMessage({ type: "cmd", bytes });
}

/// Route this frame's sound commands, draining anything buffered first so order
/// holds. Returns what should ride back to the main thread (empty on the lane).
function routeSounds(sounds) {
  const all = soundQueue.length ? soundQueue.concat(sounds ?? []) : (sounds ?? []);
  soundQueue = [];
  if (all.length === 0) return [];
  if (soundPort) {
    postToLane(all);
    return [];
  }
  if (laneExpected) {
    queueSounds(all);
    return [];
  }
  return all; // no lane is coming: the main thread plays these itself
}

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
  // worker_receive returns an envelope { msgs: VmToHost[], ents?, sounds? }.
  // Post it as ONE message per frame; transfer the entity buffer zero-copy so
  // its backing ArrayBuffer moves instead of being structured-clone copied.
  const env = worker_receive(msg);
  const ents = env.ents;
  const transfer = ents ? [ents.buffer] : [];
  const sounds = routeSounds(env.sounds);
  self.postMessage({ kind: "vm-batch", msgs: env.msgs, ents, sounds }, transfer);
}

self.onmessage = (e) => {
  const msg = e.data;
  // Routing control, handled here rather than in the VM: it's about transport,
  // and a MessagePort can't cross into wasm as a HostToVm anyway.
  if (msg && msg.kind === "sound-port") {
    soundPort = msg.port;
    laneExpected = true;
    const buffered = soundQueue;
    soundQueue = [];
    if (buffered.length) postToLane(buffered);
    return;
  }
  if (msg && msg.kind === "sound-no-lane") {
    // The worklet isn't happening; the main thread will play these. Hand back
    // whatever we were holding so nothing is lost.
    laneExpected = false;
    const buffered = soundQueue;
    soundQueue = [];
    if (buffered.length) {
      self.postMessage({ kind: "vm-batch", msgs: [], sounds: buffered });
    }
    return;
  }
  if (!ready) {
    queue.push(msg);
    return;
  }
  dispatch(msg);
};

boot().catch((err) => {
  console.error("[petrichor worker] boot failed:", err);
});

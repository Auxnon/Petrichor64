// Key-to-sound latency probe. Off unless the page is opened with `?lat`.
//
// A key press crosses three threads before it's heard: the page (winit sees
// the keydown, and the next frame posts it to the VM worker in a `Loop`), the
// worker (Lua's loop() calls note(), and the command goes down the private
// lane), and the AudioWorklet (the next render quantum mixes it in). Each stage
// stamps its part and they meet here, on one clock: performance.timeOrigin +
// now(), which every thread of the page shares. The worklet has no
// performance clock, so it reports the audio frame of the first block that
// carries the note, and getOutputTimestamp() maps that frame to the moment the
// browser expects it to leave the speaker.
//
// Per press, in ms:
//   frame  keydown -> the next Loop posted to the worker (waiting for a frame)
//   hop    Loop posted -> the worker starts on it
//   lua    the worker's loop() run, up to the note leaving for the worklet
//   lane   note sent -> rendered (the lane, plus waiting for the next quantum)
//   out    rendered -> speaker, as the browser estimates it (outputLatency)
//   total  keydown -> speaker
//
// keydown uses the event's own timeStamp, so time the main thread spent busy
// before dispatching it counts against `frame`. A sound that isn't from the
// key (a song playing by itself) pairs with the last press, so measure in an
// app that only makes sound when a key is pressed (sound-chor's piano).
//
// Loaded as a classic script before the engine's module, so the AudioContext
// and Worker patches are in place before the engine creates either.
(() => {
  if (!new URLSearchParams(location.search).has("lat")) return;

  const T0 = performance.timeOrigin;
  const now = () => T0 + performance.now();
  const bc = new BroadcastChannel("p64-lat");

  // Keep the engine's AudioContext: frame -> speaker needs its clock mapping.
  let ctx = null;
  const BaseContext = window.AudioContext;
  window.AudioContext = class extends BaseContext {
    constructor(...args) {
      super(...args);
      ctx = this;
    }
  };

  // Stamp each Loop as it's posted. The worker takes the stamp off before the
  // message reaches Rust.
  const post = Worker.prototype.postMessage;
  Worker.prototype.postMessage = function (msg, ...rest) {
    if (msg && typeof msg === "object" && "Loop" in msg) msg.__lat = now();
    return post.call(this, msg, ...rest);
  };

  let press = null; // the press being measured
  const rows = [];

  addEventListener(
    "keydown",
    (e) => {
      if (e.repeat) return;
      press = { key: e.key, down: T0 + e.timeStamp };
    },
    true,
  );

  bc.onmessage = (e) => {
    const m = e.data;
    if (!m) return;
    if (m.kind === "hello") {
      bc.postMessage({ kind: "on" });
    } else if (m.kind === "vm") {
      if (press && !press.vm && m.post > press.down) press.vm = m;
    } else if (m.kind === "worklet") {
      if (press && press.vm && ctx) finish(press, m);
    }
  };

  function finish(p, w) {
    press = null;
    const ts = ctx.getOutputTimestamp();
    const out = (ctx.outputLatency || 0) * 1000;
    const speaker = T0 + ts.performanceTime + (w.frame / ctx.sampleRate - ts.contextTime) * 1000;
    const rendered = speaker - out;
    const r = {
      key: p.key,
      frame: p.vm.post - p.down,
      hop: p.vm.recv - p.vm.post,
      lua: p.vm.done - p.vm.recv,
      lane: rendered - p.vm.done,
      out,
      total: speaker - p.down,
    };
    rows.push(r);
    const f = (v) => v.toFixed(1).padStart(6);
    const line = `key ${r.key.padEnd(3)} frame${f(r.frame)} hop${f(r.hop)} lua${f(r.lua)} lane${f(r.lane)} out${f(r.out)} | total${f(r.total)} ms`;
    console.log("[lat]", line);
    show(line);
    if (rows.length % 10 === 0) console.table(summary());
  }

  function summary() {
    const cols = ["frame", "hop", "lua", "lane", "out", "total"];
    const pick = (xs, q) => xs[Math.min(xs.length - 1, Math.floor(q * xs.length))];
    const table = {};
    for (const c of cols) {
      const xs = rows.map((r) => r[c]).sort((a, b) => a - b);
      table[c] = { median: pick(xs, 0.5), p90: pick(xs, 0.9), max: xs[xs.length - 1] };
    }
    return {
      presses: rows.length,
      baseLatency: ctx ? ctx.baseLatency * 1000 : null,
      outputLatency: ctx ? (ctx.outputLatency || 0) * 1000 : null,
      sampleRate: ctx ? ctx.sampleRate : null,
      stages: table,
    };
  }

  // A one-line readout over the game, so no devtools are needed to read it.
  let box = null;
  function show(line) {
    if (!box) {
      box = document.createElement("pre");
      box.style.cssText =
        "position:fixed;left:8px;top:8px;margin:0;padding:4px 6px;z-index:9;" +
        "font:12px monospace;color:#9f9;background:#000c;pointer-events:none";
      document.body.appendChild(box);
    }
    const s = summary().stages.total;
    box.textContent = `${line}\n${rows.length} presses, total median ${s.median.toFixed(1)} p90 ${s.p90.toFixed(1)} ms`;
  }

  window.p64lat = { rows, summary };
})();

// Petrichor64's low-latency web audio path: an AudioWorkletProcessor that runs
// the engine's synth (the `petrichor-synth` crate, compiled to its own small wasm
// module) on the browser's dedicated audio rendering thread.
//
// Why this exists: the fallback path (`sound::WebAudioOut`) generates audio on the
// MAIN thread and schedules it ahead of time, needing ~90ms of lookahead to
// survive frame hitches. Fine for playback, unusable for playing music. Here the
// browser asks us for 128 frames at a time (~2.7ms at 48kHz), so latency drops to
// the browser's own output floor.
//
// Why the module arrives by message: AudioWorkletGlobalScope has NO fetch (and no
// WebAssembly.instantiateStreaming), so a worklet cannot load wasm itself. The
// main thread compiles it and posts the WebAssembly.Module over our port. Compiled
// modules are structured-cloneable and browsers share the compiled code across
// instances, so this costs no extra download and no duplicated code — only the
// instance's own linear memory.
//
// Loaded by the engine as: ctx.audioWorklet.addModule('synth-worklet.js')

// MUST come first: the worklet scope has no TextDecoder/TextEncoder, and the glue
// below builds one at module top level. Import order is the polyfill's only chance
// to run (static imports are hoisted, so an inline polyfill would be too late).
import './worklet-polyfill.js';
// `initSync` is a *named* export of wasm-bindgen's --target web glue (the default
// export is the async initializer, which would try to fetch — impossible in here).
import { initSync, Synth } from './petrichor_synth.js';

class PetrichorSynthProcessor extends AudioWorkletProcessor {
  constructor() {
    super();
    this.synth = null;
    // Mono scratch buffer for one render quantum. Allocated once on first
    // process() call (we don't know the block size until then, though it is 128
    // in every current browser) — never per block.
    this.scratch = null;
    // Diagnostics. "No sound, no errors" is otherwise almost impossible to
    // localise from outside: these counters say whether we're being rendered at
    // all, whether commands are arriving, and whether the mixer is producing
    // signal. Reported for the first few seconds only, then silent.
    this.blocks = 0;
    this.cmds = 0;
    this.peak = 0;
    this.reportUntil = 0;
    this.lastReport = 0;
    this.port.onmessage = (e) => this.onMessage(e.data);
    // A message that arrives but fails to deserialize fires this instead of
    // onmessage — the case where a posted WebAssembly.Module doesn't survive the
    // hop to this thread. Without a handler it is completely silent on both sides.
    this.port.onmessageerror = () => {
      this.port.postMessage({
        type: 'error',
        message: 'a message from the main thread could not be deserialized',
      });
    };
    // Proof of construction. The processor is built on the audio *rendering*
    // thread, which a suspended AudioContext never starts — so this message
    // arriving is what distinguishes "the worklet never came alive" from "it came
    // alive and something later failed". Both look identical from the main thread.
    this.port.postMessage({ type: 'hello' });
  }

  onMessage(msg) {
    if (!msg) return;
    switch (msg.type) {
      case 'wasm':
        try {
          // Raw bytes, compiled here. A pre-compiled WebAssembly.Module would save
          // this work, but Chrome refuses to deserialize one on the audio thread —
          // the message fails to arrive at all (see the main thread's Stage::Loaded).
          //
          // initSync sync-compiles a BufferSource. That blocks this thread for a
          // few ms on ~289 KB, which is why it happens during the handshake, before
          // any note is sounding. (The 4 KB limit on synchronous compilation
          // applies to the main thread; we are not on it.)
          const wasm = msg.bytes;
          if (!wasm) {
            this.port.postMessage({ type: 'error', message: 'no wasm bytes in the message' });
            break;
          }
          this.port.postMessage({ type: 'got', via: `${wasm.byteLength} bytes` });
          initSync({ module: wasm });
          // Prefer the rate the main thread measured; fall back to the scope's
          // global. A non-finite rate would make every phase increment NaN and
          // the output silent-but-error-free, so refuse to build on one.
          const rate = Number.isFinite(msg.sampleRate) ? msg.sampleRate : sampleRate;
          if (!Number.isFinite(rate) || rate <= 0) {
            this.port.postMessage({
              type: 'error',
              message: `bad sampleRate (${msg.sampleRate} / ${sampleRate})`,
            });
            break;
          }
          this.synth = new Synth(rate);
          this.reportUntil = currentTime + 6;
          this.port.postMessage({ type: 'ready', rate });
        } catch (err) {
          // Report instead of throwing: an exception in here kills the audio
          // thread and silences the page with no explanation.
          this.port.postMessage({ type: 'error', message: String(err) });
        }
        break;
      case 'cmd-port':
        // A private lane for commands, owned by whoever actually produces them
        // (the VM worker). Its traffic is identical to 'cmd' below — only the
        // route differs, skipping the main thread and its frame boundary.
        if (msg.port) {
          msg.port.onmessage = (e) => this.onMessage(e.data);
          msg.port.onmessageerror = () => {
            this.port.postMessage({
              type: 'error',
              message: 'a message on the command lane could not be deserialized',
            });
          };
          this.port.postMessage({ type: 'lane' });
        }
        break;
      case 'cmd':
        // One MessagePack-encoded SoundCommand. Decoded in Rust; queued onto the
        // same mpsc the mixer drains natively.
        if (this.synth && msg.bytes) {
          try {
            this.cmds++;
            this.synth.command(msg.bytes);
          } catch (err) {
            this.port.postMessage({ type: 'error', message: String(err) });
          }
        }
        break;
      default:
        break;
    }
  }

  process(inputs, outputs) {
    const out = outputs[0];
    if (!out || out.length === 0) return true;
    const frames = out[0].length;

    // Before the module lands, emit silence rather than stale memory.
    if (!this.synth) {
      for (const channel of out) channel.fill(0);
      return true;
    }

    if (!this.scratch || this.scratch.length !== frames) {
      this.scratch = new Float32Array(frames);
    }
    this.synth.process(this.scratch);
    // The synth is mono (as on native, where write_data copies one value into
    // every frame slot); fan it out to each output channel.
    for (const channel of out) channel.set(this.scratch);

    // Diagnostics: is anything actually coming out, and does it reach the graph?
    this.blocks++;
    if (currentTime < this.reportUntil) {
      for (let i = 0; i < frames; i++) {
        const a = Math.abs(this.scratch[i]);
        if (a > this.peak) this.peak = a;
      }
      if (currentTime - this.lastReport >= 1) {
        this.lastReport = currentTime;
        this.port.postMessage({
          type: 'stats',
          blocks: this.blocks,
          cmds: this.cmds,
          peak: this.peak,
          channels: out.length,
          frames,
        });
        this.peak = 0;
      }
    }
    // Keep the node alive even while silent — the engine may start notes later.
    return true;
  }
}

registerProcessor('petrichor-synth', PetrichorSynthProcessor);

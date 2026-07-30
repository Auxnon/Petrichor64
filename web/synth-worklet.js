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
    this.port.onmessage = (e) => this.onMessage(e.data);
  }

  onMessage(msg) {
    if (!msg) return;
    switch (msg.type) {
      case 'wasm':
        try {
          // initSync takes an already-compiled module, which is exactly what we
          // have (nothing in here can fetch one).
          initSync({ module: msg.module });
          this.synth = new Synth(sampleRate);
          this.port.postMessage({ type: 'ready' });
        } catch (err) {
          // Report instead of throwing: an exception in here kills the audio
          // thread and silences the page with no explanation.
          this.port.postMessage({ type: 'error', message: String(err) });
        }
        break;
      case 'cmd':
        // One MessagePack-encoded SoundCommand. Decoded in Rust; queued onto the
        // same mpsc the mixer drains natively.
        if (this.synth && msg.bytes) {
          try {
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
    // Keep the node alive even while silent — the engine may start notes later.
    return true;
  }
}

registerProcessor('petrichor-synth', PetrichorSynthProcessor);

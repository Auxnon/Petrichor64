// Minimal TextDecoder/TextEncoder for AudioWorkletGlobalScope.
//
// The worklet scope is deliberately tiny: no window, no fetch, and — the reason
// this file exists — no TextDecoder/TextEncoder. wasm-bindgen's glue builds a
// `new TextDecoder(...)` at module top level (it marshals strings for panics and
// `__wbindgen_throw`), so without these the glue throws
// "ReferenceError: TextDecoder is not defined" while it's being evaluated, which
// fails the whole worklet module.
//
// This MUST be imported before ./petrichor_synth.js. ES modules evaluate in import
// order and static imports are hoisted, so a polyfill written inline in
// synth-worklet.js would run after the glue had already failed.
//
// Only what the glue actually uses is implemented: `decode()` (with no argument,
// which it calls once to prime the decoder, and with a Uint8Array) and `encode()`.
// `encodeInto` is deliberately absent — wasm-bindgen feature-detects it and falls
// back to `encode` + `set`, which saves implementing its read/written contract.

if (typeof globalThis.TextDecoder === 'undefined') {
  globalThis.TextDecoder = class TextDecoder {
    constructor(label = 'utf-8', options = {}) {
      this.encoding = label;
      this.fatal = !!options.fatal;
      this.ignoreBOM = !!options.ignoreBOM;
    }

    decode(input) {
      if (input === undefined) return '';
      const bytes =
        input instanceof Uint8Array
          ? input
          : new Uint8Array(input.buffer || input, input.byteOffset || 0, input.byteLength);
      let out = '';
      let i = 0;
      while (i < bytes.length) {
        const b = bytes[i++];
        if (b < 0x80) {
          out += String.fromCharCode(b);
        } else if (b < 0xe0) {
          out += String.fromCharCode(((b & 0x1f) << 6) | (bytes[i++] & 0x3f));
        } else if (b < 0xf0) {
          out += String.fromCharCode(
            ((b & 0x0f) << 12) | ((bytes[i++] & 0x3f) << 6) | (bytes[i++] & 0x3f)
          );
        } else {
          // Astral plane: decode to a surrogate pair.
          const cp =
            ((b & 0x07) << 18) |
            ((bytes[i++] & 0x3f) << 12) |
            ((bytes[i++] & 0x3f) << 6) |
            (bytes[i++] & 0x3f);
          const c = cp - 0x10000;
          out += String.fromCharCode(0xd800 + (c >> 10), 0xdc00 + (c & 0x3ff));
        }
      }
      return out;
    }
  };
}

if (typeof globalThis.TextEncoder === 'undefined') {
  globalThis.TextEncoder = class TextEncoder {
    constructor() {
      this.encoding = 'utf-8';
    }

    encode(str = '') {
      const out = [];
      for (let i = 0; i < str.length; i++) {
        let cp = str.codePointAt(i);
        if (cp > 0xffff) i++; // consumed a surrogate pair
        if (cp < 0x80) {
          out.push(cp);
        } else if (cp < 0x800) {
          out.push(0xc0 | (cp >> 6), 0x80 | (cp & 0x3f));
        } else if (cp < 0x10000) {
          out.push(0xe0 | (cp >> 12), 0x80 | ((cp >> 6) & 0x3f), 0x80 | (cp & 0x3f));
        } else {
          out.push(
            0xf0 | (cp >> 18),
            0x80 | ((cp >> 12) & 0x3f),
            0x80 | ((cp >> 6) & 0x3f),
            0x80 | (cp & 0x3f)
          );
        }
      }
      return new Uint8Array(out);
    }
  };
}

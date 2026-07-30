//! Petrichor64's synth, split out so the engine and the browser AudioWorklet can
//! share one implementation.
//!
//! - [`fx`] — reusable DSP primitives (crossfade, biquad/filter, echo, reverb,
//!   bitcrush, drive).
//! - [`vocaloid`] — the formant-synthesis singing voice.
//! - [`sound`] — the mixer itself (channels, lanes, voices, `SoundCommand`), plus
//!   the host audio I/O behind the `host-io` feature.
//!
//! The DSP is I/O-free by default. `host-io` adds the cpal output stream and
//! microphone capture (and the main-thread WebAudio fallback) that the engine
//! needs; the AudioWorklet build leaves it off, so none of that is compiled in and
//! the wasm module stays small.

pub mod fx;
pub mod sound;
pub mod vocaloid;
// The synth's side of the low-latency browser path: what runs *inside* the
// AudioWorklet. Only for the wasm-pack build of this crate (`just synth-wasm`),
// which is the module the worklet loads — hence its own feature rather than plain
// wasm32. The engine can never call `Synth`, so compiling it there just anchored
// the entire DSP as live code in a bundle that reaches it another way.
#[cfg(all(feature = "worklet", target_arch = "wasm32"))]
pub mod worklet;
// The engine's side: sets the worklet up and forwards commands to it, falling
// back to the main-thread scheduler in `sound.rs` if that can't be done.
#[cfg(all(feature = "host-io", target_arch = "wasm32"))]
pub mod webout;

// The engine refers to these as `crate::sound::X` / `crate::fx::X` today, and the
// shim modules left behind in the engine re-export from here, so nothing at the
// call sites had to change.
pub use sound::*;

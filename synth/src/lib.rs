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

// The engine refers to these as `crate::sound::X` / `crate::fx::X` today, and the
// shim modules left behind in the engine re-export from here, so nothing at the
// call sites had to change.
pub use sound::*;

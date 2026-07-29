//! The synth lives in the `petrichor-synth` crate so the engine and the browser
//! AudioWorklet can share one implementation (see `synth/src/lib.rs`). This shim
//! keeps the engine's existing `crate::sound::…` paths working unchanged.
pub use petrichor_synth::sound::*;

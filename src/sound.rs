//! The synth lives in the `petrichor-synth` crate so the engine and the browser
//! AudioWorklet can share one implementation (see `synth/src/lib.rs`). This shim
//! keeps the engine's existing `crate::sound::…` paths working unchanged.
pub use petrichor_synth::sound::*;

// Browser output: the AudioWorklet path, with the main-thread scheduler in
// `sound.rs` as its fallback. `init_web` here shadows the one re-exported above.
#[cfg(target_arch = "wasm32")]
pub use petrichor_synth::webout::{init_web, WebOut};

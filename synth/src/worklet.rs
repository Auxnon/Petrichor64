//! The synth's entry points for a browser **AudioWorklet** — the low-latency web
//! audio path.
//!
//! Why a worklet: the engine's fallback (`sound::WebAudioOut`) generates audio on
//! the *main thread* and schedules it ahead of time, which needs ~90 ms of
//! lookahead to survive frame hitches — fine for playback, far too laggy to play
//! music with. An AudioWorklet runs on the browser's dedicated audio rendering
//! thread and asks for a **128-frame** block at a time (~2.7 ms at 48 kHz), so
//! latency drops to the browser's own output floor (~15–30 ms round trip).
//!
//! Why this lives in its own small crate: `AudioWorkletGlobalScope` has no
//! `fetch`, so the worklet cannot load a wasm module itself — the main thread must
//! compile it and hand it over by `postMessage`. Instantiating the *engine's* 3.4 MB
//! module a second time just to reach the mixer would be absurd, so the synth is
//! built as its own artifact (see `lib.rs`) with the host I/O feature off.
//!
//! Lifecycle, all driven from the JS processor in `web/synth-worklet.js`:
//! 1. `Synth::new(sample_rate)` once, when the processor is constructed.
//! 2. `command(bytes)` for each MessagePack-encoded [`SoundCommand`] that arrives
//!    on the processor's port.
//! 3. `process(&mut [f32])` per render quantum, filling the output block.
//!
//! Nothing here allocates per block: the mixer's buffers were sized at
//! construction (see `Echo::new`/`Reverb::new`), and commands are decoded on
//! arrival, not in `process`.

use std::sync::mpsc::{channel, Sender};

use wasm_bindgen::prelude::*;

use crate::sound::{make_mixer_boxed, SoundCommand};

/// A synth instance owned by one `AudioWorkletProcessor`.
#[wasm_bindgen]
pub struct Synth {
    mixer: Box<dyn FnMut() -> f32>,
    /// Commands are pushed here and drained by the mixer, exactly as on native —
    /// the message port simply replaces the engine's `mpsc` producer.
    sender: Sender<SoundCommand>,
}

#[wasm_bindgen]
impl Synth {
    /// Build the synth for the worklet's sample rate (`sampleRate` in the
    /// `AudioWorkletGlobalScope`).
    #[wasm_bindgen(constructor)]
    pub fn new(sample_rate: f32) -> Synth {
        let (sender, audience) = channel::<SoundCommand>();
        Synth {
            mixer: make_mixer_boxed(sample_rate, audience),
            sender,
        }
    }

    /// Decode one MessagePack-encoded `SoundCommand` and queue it for the mixer.
    /// Malformed input is dropped rather than panicking — a panic here would take
    /// down the audio thread and silence the page.
    pub fn command(&mut self, bytes: &[u8]) {
        if let Ok(cmd) = rmp_serde::from_slice::<SoundCommand>(bytes) {
            let _ = self.sender.send(cmd);
        }
    }

    /// Fill one render quantum (mono). The JS side copies this into every output
    /// channel, matching the native path's mono-to-all-channels behaviour.
    pub fn process(&mut self, out: &mut [f32]) {
        for s in out.iter_mut() {
            *s = (self.mixer)();
        }
    }
}

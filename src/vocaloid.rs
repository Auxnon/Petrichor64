//! Retro formant-synthesis voice ("vocaloid"), layered on the sound engine.
//!
//! A sung note is a harmonic-rich glottal source (a sawtooth at the note pitch)
//! shaped by a bank of resonant band-pass filters — the *formants* — that carve
//! it into a vowel. This is how old speech chips (SAM, Votrax) sang: no recorded
//! voice, just a source + formant filters. `sound.rs` owns the source + note
//! timing; this module owns the filters, the bank, and the vowel table.

use serde::{Deserialize, Serialize};

const TWO_PI: f32 = std::f32::consts::PI * 2.0;

/// One formant: a resonance at `freq` Hz with quality `q` (sharpness) and output
/// `gain`. A vowel is three of these. Serializable so a sung `Note` can carry it
/// across the wasm worker→main boundary.
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
pub struct Formant {
    pub freq: f32,
    pub q: f32,
    pub gain: f32,
}

/// A resonant band-pass biquad (transposed direct-form II). Coefficients are
/// baked once from (freq, q, sample_rate); `z1`/`z2` are the per-voice state, so
/// each sounding voice needs its own. Default is a silent/pass-nothing filter.
#[derive(Clone, Copy, Default)]
pub struct Biquad {
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,
    z1: f32,
    z2: f32,
    gain: f32,
}

impl Biquad {
    /// Band-pass (constant 0 dB peak) at `freq` with quality `q`, scaled by `gain`.
    pub fn bandpass(freq: f32, q: f32, gain: f32, sample_rate: f32) -> Self {
        let w0 = TWO_PI * freq / sample_rate;
        let (sin_w0, cos_w0) = w0.sin_cos();
        let alpha = sin_w0 / (2.0 * q.max(0.001));
        let a0 = 1.0 + alpha;
        Self {
            b0: alpha / a0,
            b1: 0.0,
            b2: -alpha / a0,
            a1: -2.0 * cos_w0 / a0,
            a2: (1.0 - alpha) / a0,
            z1: 0.0,
            z2: 0.0,
            gain,
        }
    }

    /// Filter one input sample, advancing the state.
    pub fn process(&mut self, x: f32) -> f32 {
        let y = self.b0 * x + self.z1;
        self.z1 = self.b1 * x + self.z2 - self.a1 * y;
        self.z2 = self.b2 * x - self.a2 * y;
        y * self.gain
    }
}

/// A per-voice bank of up to three formant filters summed in parallel. Inactive
/// until `set` is called with a vowel's formants, so plain notes skip it.
#[derive(Clone, Copy, Default)]
pub struct FormantBank {
    filters: [Biquad; 3],
    active: bool,
}

impl FormantBank {
    /// Bake the filters for a vowel's formants at the given sample rate.
    pub fn set(&mut self, formants: &[Formant; 3], sample_rate: f32) {
        for (bq, f) in self.filters.iter_mut().zip(formants.iter()) {
            *bq = Biquad::bandpass(f.freq, f.q, f.gain, sample_rate);
        }
        self.active = true;
    }

    pub fn is_active(&self) -> bool {
        self.active
    }

    /// Run the glottal source `src` through all formants and sum them.
    pub fn process(&mut self, src: f32) -> f32 {
        let mut out = 0.0;
        for bq in self.filters.iter_mut() {
            out += bq.process(src);
        }
        out
    }
}

/// Formant table for the five cardinal vowels (roughly male-voice values, Hz).
/// Unknown names fall back to `a`. Q/gain are tuned for a clear-but-buzzy retro
/// timbre; tweak to taste. F1 carries the body, F2 the "color", F3 a little air.
pub fn vowel_formants(vowel: &str) -> [Formant; 3] {
    let f = |freq, q, gain| Formant { freq, q, gain };
    // Match the first char so "la"/"aah"/"a" all read as the vowel.
    let v = vowel.trim().chars().next().unwrap_or('a').to_ascii_lowercase();
    match v {
        'e' => [f(400.0, 8.0, 1.0), f(1600.0, 10.0, 0.5), f(2700.0, 12.0, 0.3)],
        'i' => [f(270.0, 8.0, 1.0), f(2300.0, 11.0, 0.5), f(3000.0, 13.0, 0.3)],
        'o' => [f(400.0, 8.0, 1.0), f(800.0, 9.0, 0.6), f(2600.0, 12.0, 0.3)],
        'u' => [f(300.0, 8.0, 1.0), f(870.0, 9.0, 0.5), f(2240.0, 11.0, 0.3)],
        // 'a' and anything else
        _ => [f(800.0, 8.0, 1.0), f(1150.0, 9.0, 0.6), f(2900.0, 12.0, 0.3)],
    }
}

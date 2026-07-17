//! Retro formant-synthesis voice ("vocaloid"), layered on the sound engine.
//!
//! A sung note is a harmonic-rich glottal source (a sawtooth at the note pitch)
//! shaped by a bank of resonant band-pass filters — the *formants* — that carve
//! it into a vowel. This is how old speech chips (SAM, Votrax) sang: no recorded
//! voice, just a source + formant filters. `sound.rs` owns the source + note
//! timing; this module owns the filters, the bank, and the vowel table.

use serde::{Deserialize, Serialize};

use crate::fx::Crossfade;

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
        let mut bq = Self::default();
        bq.retune(freq, q, gain, sample_rate);
        bq
    }

    /// Recompute the band-pass coefficients in place, **preserving** the filter
    /// state (z1/z2). Lets a formant sweep continuously as its frequency glides
    /// without the click a fresh filter (zeroed state) would cause.
    pub fn retune(&mut self, freq: f32, q: f32, gain: f32, sample_rate: f32) {
        let w0 = TWO_PI * freq / sample_rate;
        let (sin_w0, cos_w0) = w0.sin_cos();
        let alpha = sin_w0 / (2.0 * q.max(0.001));
        let a0 = 1.0 + alpha;
        self.b0 = alpha / a0;
        self.b1 = 0.0;
        self.b2 = -alpha / a0;
        self.a1 = -2.0 * cos_w0 / a0;
        self.a2 = (1.0 - alpha) / a0;
        self.gain = gain;
    }

    /// Filter one input sample, advancing the state.
    pub fn process(&mut self, x: f32) -> f32 {
        let y = self.b0 * x + self.z1;
        self.z1 = self.b1 * x + self.z2 - self.a1 * y;
        self.z2 = self.b2 * x - self.a2 * y;
        y * self.gain
    }
}

/// A per-voice bank of up to three formant filters summed in parallel. Can hold
/// a steady vowel or glide between two formant sets (for voiced consonants and
/// diphthongs) — the filters are retuned each sample toward the target while a
/// crossfade ramp runs. Inactive until `set`/`glide_to`, so plain notes skip it.
#[derive(Clone, Copy, Default)]
pub struct FormantBank {
    filters: [Biquad; 3],
    from: [Formant; 3],
    to: [Formant; 3],
    glide: Crossfade,
    sr: f32,
    active: bool,
}

impl FormantBank {
    /// Set a steady vowel (no glide).
    pub fn set(&mut self, formants: &[Formant; 3], sample_rate: f32) {
        self.from = *formants;
        self.to = *formants;
        self.sr = sample_rate;
        for (bq, f) in self.filters.iter_mut().zip(formants.iter()) {
            bq.retune(f.freq, f.q, f.gain, sample_rate);
        }
        self.glide = Crossfade::default(); // already at target
        self.active = true;
    }

    /// Start at `from` and glide the formants to `to` over `secs` — a voiced
    /// consonant sliding into its vowel, or a diphthong (vowel→vowel).
    pub fn glide_to(&mut self, from: &[Formant; 3], to: &[Formant; 3], secs: f32, sample_rate: f32) {
        self.from = *from;
        self.to = *to;
        self.sr = sample_rate;
        for (bq, f) in self.filters.iter_mut().zip(from.iter()) {
            bq.retune(f.freq, f.q, f.gain, sample_rate);
        }
        self.glide = Crossfade::new(secs, sample_rate);
        self.active = true;
    }

    pub fn is_active(&self) -> bool {
        self.active
    }

    /// Run the glottal source `src` through all formants and sum them, advancing
    /// any in-flight glide (retuning each formant toward its target).
    pub fn process(&mut self, src: f32) -> f32 {
        if !self.glide.done() {
            let t = self.glide.mix(0.0, 1.0); // progress 0→1, advances the ramp
            for i in 0..3 {
                let (a, b) = (self.from[i], self.to[i]);
                self.filters[i].retune(
                    a.freq + (b.freq - a.freq) * t,
                    a.q + (b.q - a.q) * t,
                    a.gain + (b.gain - a.gain) * t,
                    self.sr,
                );
            }
        }
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
        'o' => [f(450.0, 8.0, 1.0), f(800.0, 9.0, 0.6), f(2600.0, 12.0, 0.3)],
        // u ("oo") needs F2 clearly *below* o's, or it just reads as another o.
        'u' => [f(320.0, 9.0, 1.0), f(620.0, 10.0, 0.5), f(2200.0, 11.0, 0.25)],
        // 'a' and anything else
        _ => [f(800.0, 8.0, 1.0), f(1150.0, 9.0, 0.6), f(2900.0, 12.0, 0.3)],
    }
}

/// An *unvoiced* consonant onset: a short burst of band-passed noise before the
/// vowel — fricatives s/f/h/sh and plosives t/k/p (voiced pairs z/v/d/g/b
/// approximated the same). *Voiced* consonants (m/n/l/r/w/y) are handled instead
/// as a voiced formant onset that glides into the vowel (see `voiced_onset`).
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Consonant {
    pub freq: f32,
    pub q: f32,
    /// Onset length in seconds.
    pub secs: f32,
    pub gain: f32,
}

/// A parsed sung syllable: an optional unvoiced noise onset, an optional starting
/// formant set to glide *from* (a voiced consonant, or a diphthong's first
/// vowel), and the vowel formants to land on.
pub struct SungSyllable {
    pub consonant: Option<Consonant>,
    pub glide_from: Option<[Formant; 3]>,
    pub glide_secs: f32,
    pub vowel: [Formant; 3],
}

/// Formant target for a *voiced* consonant (m/n/l/r/w/y). These are tonal (use
/// the glottal source) and glide into the following vowel.
fn voiced_onset(c: char) -> Option<[Formant; 3]> {
    let f = |freq, q, gain| Formant { freq, q, gain };
    match c {
        'm' => Some([f(250.0, 10.0, 1.0), f(1000.0, 10.0, 0.4), f(2200.0, 12.0, 0.2)]), // nasal
        'n' => Some([f(250.0, 10.0, 1.0), f(1400.0, 11.0, 0.5), f(2500.0, 12.0, 0.2)]), // nasal
        'l' => Some([f(360.0, 9.0, 1.0), f(1300.0, 10.0, 0.6), f(2800.0, 12.0, 0.3)]),  // lateral
        'r' => Some([f(490.0, 9.0, 1.0), f(1350.0, 10.0, 0.6), f(1700.0, 11.0, 0.4)]),  // low F3
        'w' => Some([f(300.0, 9.0, 1.0), f(610.0, 10.0, 0.5), f(2200.0, 11.0, 0.25)]),  // ~/u/
        'y' => Some([f(270.0, 9.0, 1.0), f(2300.0, 11.0, 0.5), f(3000.0, 12.0, 0.25)]), // ~/i/
        _ => None,
    }
}

/// Map a leading character to its consonant onset, if any.
fn consonant_for(c: char) -> Option<Consonant> {
    let cons = |freq, q, secs, gain| {
        Some(Consonant {
            freq,
            q,
            secs,
            gain,
        })
    };
    match c {
        's' | 'z' => cons(6000.0, 2.0, 0.08, 0.5), // hiss
        'f' | 'v' => cons(4000.0, 1.0, 0.08, 0.45), // broadband
        'h' => cons(1500.0, 0.7, 0.06, 0.4),       // breathy
        't' | 'd' => cons(4000.0, 1.5, 0.02, 0.7), // sharp tick
        'k' | 'g' => cons(2000.0, 1.5, 0.02, 0.7), // mid pop
        'p' | 'b' => cons(800.0, 1.2, 0.02, 0.7),  // low pop
        _ => None, // vowels + l/r/m/n/w/y
    }
}

/// Parse a sung syllable. Examples:
/// - `"sa"`  → unvoiced hiss onset, vowel /a/
/// - `"la"`  → voiced /l/ formants gliding into /a/
/// - `"ma"`  → voiced nasal /m/ gliding into /a/
/// - `"ai"`  → diphthong: glide /a/ → /i/
/// - `"o"`   → steady /o/
pub fn parse_syllable(s: &str) -> SungSyllable {
    let lower = s.trim().to_lowercase();

    let mut consonant = None;
    let mut glide_from = None;
    let mut glide_secs = 0.0;

    // Leading consonant: digraph first, then a single char (unvoiced noise or
    // voiced glide).
    if lower.starts_with("sh") || lower.starts_with("ch") {
        consonant = Some(Consonant {
            freq: 3000.0,
            q: 1.2,
            secs: 0.09,
            gain: 0.5,
        });
    } else if let Some(first) = lower.chars().next() {
        if let Some(k) = consonant_for(first) {
            consonant = Some(k);
        } else if let Some(vf) = voiced_onset(first) {
            glide_from = Some(vf); // voiced consonant → glide into the vowel
            glide_secs = 0.06;
        }
    }

    // Vowels present, in order.
    let vowels: Vec<char> = lower.chars().filter(|c| "aeiou".contains(*c)).collect();
    let last = vowels.last().copied().unwrap_or('a');
    let vowel = vowel_formants(&last.to_string());

    // Diphthong: two different vowels and no voiced-consonant glide already —
    // glide from the first vowel to the last over a longer window.
    if glide_from.is_none() && vowels.len() >= 2 && vowels[0] != last {
        glide_from = Some(vowel_formants(&vowels[0].to_string()));
        glide_secs = 0.15;
    }

    SungSyllable {
        consonant,
        glide_from,
        glide_secs,
        vowel,
    }
}

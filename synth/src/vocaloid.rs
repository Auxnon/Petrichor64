//! Retro formant-synthesis voice ("vocaloid"), layered on the sound engine.
//!
//! A sung note is a harmonic-rich glottal source (a sawtooth at the note pitch)
//! shaped by a bank of resonant band-pass filters — the *formants* — that carve
//! it into a vowel. This is how old speech chips (SAM, Votrax) sang: no recorded
//! voice, just a source + formant filters. `sound.rs` owns the source + note
//! timing; this module owns the filters, the bank, and the vowel table.

use serde::{Deserialize, Serialize};

use crate::fx::{Biquad, Crossfade};

/// One formant: a resonance at `freq` Hz with quality `q` (sharpness) and output
/// `gain`. A vowel is three of these. Serializable so a sung `Note` can carry it
/// across the wasm worker→main boundary.
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
pub struct Formant {
    pub freq: f32,
    pub q: f32,
    pub gain: f32,
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
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
pub struct Consonant {
    pub freq: f32,
    pub q: f32,
    /// Onset length in seconds.
    pub secs: f32,
    pub gain: f32,
}

/// Most bursts a [`Cluster`] holds. Four covers anything worth pronouncing —
/// "str" contributes two unvoiced bursts, "nts" three.
pub const CLUSTER_MAX: usize = 4;

/// A consonant cluster: up to [`CLUSTER_MAX`] bursts, stored inline.
///
/// Fixed-size deliberately, where a `Vec` would read more naturally. These ride
/// inside a `Note`, which is handed to the audio thread and **dropped there** — so
/// a `Vec` meant every sung note allocated on the engine side and called `free()`
/// inside the audio callback, the one place that must never wait on the allocator.
/// (Plain notes were always safe: an empty `Vec` doesn't allocate. Only singing
/// tripped it.) Real clusters are short, so an inline array costs a few bytes and
/// removes the hazard outright — and keeps `Note` `Copy`.
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
pub struct Cluster {
    bursts: [Consonant; CLUSTER_MAX],
    len: u8,
}

impl Cluster {
    pub const MAX: usize = CLUSTER_MAX;

    /// Append a burst, dropping any past [`CLUSTER_MAX`]. A syllable with five
    /// stacked consonants isn't something we need to pronounce, and quietly losing
    /// the tail beats refusing to sing the word.
    pub fn push(&mut self, burst: Consonant) {
        if (self.len as usize) < CLUSTER_MAX {
            self.bursts[self.len as usize] = burst;
            self.len += 1;
        }
    }

    pub fn len(&self) -> usize {
        self.len as usize
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn iter(&self) -> std::slice::Iter<'_, Consonant> {
        self.bursts[..self.len as usize].iter()
    }
}

impl std::ops::Index<usize> for Cluster {
    type Output = Consonant;
    fn index(&self, i: usize) -> &Consonant {
        &self.bursts[..self.len as usize][i]
    }
}

/// A parsed sung syllable, in playback order: an **onset** cluster of unvoiced
/// noise bursts, an optional voiced onset to glide *from* (a voiced consonant or
/// a diphthong's first vowel), the **vowel** to land on, an optional voiced
/// **coda** to glide *to* at the end (a nasal/liquid ending like `sun`/`call`),
/// then a **coda** cluster of unvoiced bursts (`cat`, `cats`). Lets whole
/// CVC(C) words sing, not just CV syllables.
pub struct SungSyllable {
    pub onset: Cluster,
    pub glide_from: Option<[Formant; 3]>,
    pub glide_secs: f32,
    pub vowel: [Formant; 3],
    pub coda_glide: Option<[Formant; 3]>,
    pub coda: Cluster,
}

/// One consonant in a cluster: an unvoiced noise burst, or a voiced (nasal /
/// liquid / glide) formant set that the vowel glides through.
enum Cons {
    Unvoiced(Consonant),
    Voiced([Formant; 3]),
}

/// Split a run of consonant characters into ordered tokens, handling the common
/// digraphs: `sh`/`ch` (hiss), `ng` (nasal), `ck` (→k), `th` (→f-ish). Used for
/// both the onset and coda clusters, so `str`/`spl`/`nts`/`nk` all decompose.
fn parse_consonants(s: &str) -> Vec<Cons> {
    let sh = Consonant { freq: 3000.0, q: 1.2, secs: 0.09, gain: 0.5 };
    let chars: Vec<char> = s.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        let next = chars.get(i + 1).copied();
        match (c, next) {
            ('s', Some('h')) | ('c', Some('h')) => {
                out.push(Cons::Unvoiced(sh));
                i += 2;
            }
            ('n', Some('g')) => {
                if let Some(vf) = voiced_onset('n') {
                    out.push(Cons::Voiced(vf)); // nasal
                }
                i += 2;
            }
            ('c', Some('k')) => {
                if let Some(k) = consonant_for('k') {
                    out.push(Cons::Unvoiced(k));
                }
                i += 2;
            }
            ('t', Some('h')) => {
                if let Some(k) = consonant_for('f') {
                    out.push(Cons::Unvoiced(k)); // approximate 'th' as a soft fricative
                }
                i += 2;
            }
            _ => {
                if let Some(k) = consonant_for(c) {
                    out.push(Cons::Unvoiced(k));
                } else if let Some(vf) = voiced_onset(c) {
                    out.push(Cons::Voiced(vf));
                }
                i += 1;
            }
        }
    }
    out
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

/// Parse a sung syllable into onset / vowel / coda. Examples:
/// - `"sa"`   → unvoiced hiss onset, vowel /a/
/// - `"la"`   → voiced /l/ formants gliding into /a/
/// - `"ai"`   → diphthong: glide /a/ → /i/
/// - `"stra"` → onset cluster s+t bursts, /r/ glide, vowel /a/
/// - `"cat"`  → onset /k/, vowel /a/, coda /t/ burst
/// - `"sun"`  → onset /s/, vowel /u/, voiced /n/ coda glide
/// - `"sink"` → /s/, /i/, /n/ coda glide + /k/ burst
pub fn parse_syllable(s: &str) -> SungSyllable {
    let lower = s.trim().to_lowercase();
    let chars: Vec<char> = lower.chars().collect();
    let is_vowel = |c: char| "aeiou".contains(c);

    // Split into onset (leading consonants) | nucleus (the vowel run) | coda
    // (trailing consonants, up to the next vowel — one syllable per token).
    let (onset_str, nucleus, coda_str) = match chars.iter().position(|&c| is_vowel(c)) {
        Some(start) => {
            let mut end = start;
            while end < chars.len() && is_vowel(chars[end]) {
                end += 1;
            }
            let mut ce = end;
            while ce < chars.len() && !is_vowel(chars[ce]) {
                ce += 1;
            }
            (
                chars[..start].iter().collect::<String>(),
                chars[start..end].to_vec(),
                chars[end..ce].iter().collect::<String>(),
            )
        }
        // No vowel: sing the consonants over a default /a/.
        None => (lower.clone(), vec!['a'], String::new()),
    };

    // Onset: unvoiced bursts, plus an optional voiced glide-from (last one wins).
    let mut onset = Cluster::default();
    let mut glide_from = None;
    let mut glide_secs = 0.0;
    for tok in parse_consonants(&onset_str) {
        match tok {
            Cons::Unvoiced(k) => onset.push(k),
            Cons::Voiced(vf) => {
                glide_from = Some(vf);
                glide_secs = 0.06;
            }
        }
    }

    // Vowel nucleus, and a diphthong glide if the run holds two different vowels
    // (only when a voiced onset glide didn't already claim the glide).
    let last = nucleus.last().copied().unwrap_or('a');
    let vowel = vowel_formants(&last.to_string());
    if glide_from.is_none() && nucleus.len() >= 2 && nucleus[0] != last {
        glide_from = Some(vowel_formants(&nucleus[0].to_string()));
        glide_secs = 0.15;
    }

    // Coda: an optional voiced ending to glide *to* (first voiced — a nasal/
    // liquid tail), then any unvoiced bursts (`t`, `s`, `k`…).
    let mut coda_glide = None;
    let mut coda = Cluster::default();
    for tok in parse_consonants(&coda_str) {
        match tok {
            Cons::Voiced(vf) => {
                if coda_glide.is_none() {
                    coda_glide = Some(vf);
                }
            }
            Cons::Unvoiced(k) => coda.push(k),
        }
    }

    SungSyllable {
        onset,
        glide_from,
        glide_secs,
        vowel,
        coda_glide,
        coda,
    }
}

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

/// How loud a sung vowel comes out (rms of the formant bank's output at volume 1),
/// whatever the vowel and pitch. See [`voiced_level`].
pub const VOWEL_RMS: f32 = 0.13;

/// The narrowest each formant may be (Hz), and the narrowest relative to the
/// note's pitch. Narrow resonances make a vowel's loudness depend on whether a
/// harmonic happens to land on a peak; a band at least ~0.6 × f0 wide always
/// holds one.
const MIN_BW: [f32; 3] = [80.0, 100.0, 140.0];
const MIN_BW_F0: f32 = 0.6;

/// Fit a vowel's formants to the note it's sung on, the way a singer does:
/// - F1 never sits below the fundamental (~1.1 × f0). A high /i/ or /u/ has its
///   F1 at 270–320 Hz; sung at 520 Hz that resonance has no harmonic to ring,
///   the vowel all but vanishes and only noise is left. Sopranos raise F1 to
///   follow the pitch for the same reason. F2 keeps clear of the raised F1.
/// - Each formant is at least [`MIN_BW`] wide (and 0.6 × f0), so the vowel
///   doesn't flicker between loud and silent as the melody moves.
pub fn fit_to_pitch(formants: &[Formant; 3], f0: f32) -> [Formant; 3] {
    let mut out = *formants;
    if f0 > 0.0 {
        let f1_floor = f0 * 1.1;
        if out[0].freq < f1_floor {
            out[0].freq = f1_floor;
        }
        if out[1].freq < out[0].freq * 1.3 {
            out[1].freq = out[0].freq * 1.3;
        }
    }
    for (i, f) in out.iter_mut().enumerate() {
        let min_bw = MIN_BW[i].max(f0 * MIN_BW_F0);
        let max_q = f.freq / min_bw;
        if f.q > max_q {
            f.q = max_q.max(0.5);
        }
    }
    out
}

/// The output gain that makes a sawtooth at `f0` through these (parallel,
/// constant-peak band-pass) formants come out at [`VOWEL_RMS`]. Worked out from
/// the filters' response at each harmonic — once per note, not per sample.
pub fn voiced_level(formants: &[Formant; 3], f0: f32, sample_rate: f32) -> f32 {
    if f0 <= 0.0 {
        return 1.0;
    }
    // Band-pass coefficients (as Biquad::retune), evaluated on the unit circle.
    let mut coef = [(0.0f32, 0.0f32, 0.0f32, 0.0f32); 3];
    for (c, f) in coef.iter_mut().zip(formants.iter()) {
        let w0 = std::f32::consts::TAU * f.freq / sample_rate;
        let (sin_w0, cos_w0) = w0.sin_cos();
        let alpha = sin_w0 / (2.0 * f.q.max(0.001));
        let a0 = 1.0 + alpha;
        *c = (alpha / a0, -2.0 * cos_w0 / a0, (1.0 - alpha) / a0, f.gain);
    }
    let top = (sample_rate * 0.45).min(12_000.0);
    let mut power = 0.0f32;
    let mut n = 1;
    loop {
        let hz = f0 * n as f32;
        if hz > top {
            break;
        }
        let w = std::f32::consts::TAU * hz / sample_rate;
        let (s1, c1) = w.sin_cos();
        let (s2, c2) = (2.0 * w).sin_cos();
        // H(z) = b0 (1 - z^-2) / (1 + a1 z^-1 + a2 z^-2), z^-k = cos(kw) - j sin(kw)
        let (mut re, mut im) = (0.0f32, 0.0f32);
        for &(b0, a1, a2, g) in coef.iter() {
            let (nr, ni) = (b0 * (1.0 - c2), b0 * s2);
            let (dr, di) = (1.0 + a1 * c1 + a2 * c2, -(a1 * s1 + a2 * s2));
            let d = dr * dr + di * di;
            re += g * (nr * dr + ni * di) / d;
            im += g * (ni * dr - nr * di) / d;
        }
        // A -1..1 sawtooth's nth harmonic has amplitude 2/(πn).
        let amp = 2.0 / (std::f32::consts::PI * n as f32);
        power += amp * amp * (re * re + im * im) * 0.5;
        n += 1;
    }
    if power <= 1e-12 {
        return 1.0;
    }
    (VOWEL_RMS / power.sqrt()).min(20.0)
}

/// A per-voice bank of up to three formant filters summed in parallel. Can hold
/// a steady vowel or glide between two formant sets (for voiced consonants and
/// diphthongs) — the filters are retuned each sample toward the target while a
/// crossfade ramp runs. Inactive until `set`/`glide_to`, so plain notes skip it.
///
/// Both take the note's pitch: the formants are fitted to it ([`fit_to_pitch`])
/// and the output is levelled ([`voiced_level`]) so every vowel, at every pitch,
/// comes out equally loud. The level glides along with the formants.
#[derive(Clone, Copy, Default)]
pub struct FormantBank {
    filters: [Biquad; 3],
    from: [Formant; 3],
    to: [Formant; 3],
    glide: Crossfade,
    sr: f32,
    active: bool,
    level_from: f32,
    level_to: f32,
    level: f32,
    /// The pitch the bank is fitted to.
    f0: f32,
    /// Where the formants are right now (mid-glide or landed).
    cur: [Formant; 3],
    /// A second glide to start when this one lands: (target, seconds).
    then: Option<([Formant; 3], f32)>,
}

impl FormantBank {
    /// Set a steady vowel (no glide), sung at `f0`.
    pub fn set(&mut self, formants: &[Formant; 3], f0: f32, sample_rate: f32) {
        let fitted = fit_to_pitch(formants, f0);
        self.from = fitted;
        self.to = fitted;
        self.sr = sample_rate;
        for (bq, f) in self.filters.iter_mut().zip(fitted.iter()) {
            bq.retune(f.freq, f.q, f.gain, sample_rate);
        }
        self.level = voiced_level(&fitted, f0, sample_rate);
        self.level_from = self.level;
        self.level_to = self.level;
        self.glide = Crossfade::default(); // already at target
        self.active = true;
        self.f0 = f0;
        self.cur = fitted;
        self.then = None;
    }

    /// Start at `from` and glide the formants to `to` over `secs` — a voiced
    /// consonant sliding into its vowel, or a diphthong (vowel→vowel).
    pub fn glide_to(
        &mut self,
        from: &[Formant; 3],
        to: &[Formant; 3],
        secs: f32,
        f0: f32,
        sample_rate: f32,
    ) {
        self.from = fit_to_pitch(from, f0);
        self.to = fit_to_pitch(to, f0);
        self.sr = sample_rate;
        for (bq, f) in self.filters.iter_mut().zip(self.from.iter()) {
            bq.retune(f.freq, f.q, f.gain, sample_rate);
        }
        self.level_from = voiced_level(&self.from, f0, sample_rate);
        self.level_to = voiced_level(&self.to, f0, sample_rate);
        self.level = self.level_from;
        self.glide = Crossfade::new(secs, sample_rate);
        self.active = true;
        self.f0 = f0;
        self.cur = self.from;
        self.then = None;
    }

    /// Glide from wherever the formants are now to `via` over `t1`, then on to
    /// `to` over `t2`, at a (possibly new) pitch `f0`. The filters keep ringing
    /// through it — this is how one syllable runs into the next instead of the
    /// voice stopping and a new one starting.
    pub fn glide_on(
        &mut self,
        via: Option<&[Formant; 3]>,
        t1: f32,
        to: &[Formant; 3],
        t2: f32,
        f0: f32,
    ) {
        match via {
            Some(v) => {
                self.glide_from_here(v, t1, f0);
                self.then = Some((*to, t2));
            }
            None => self.glide_from_here(to, t1, f0),
        }
    }

    /// Glide from exactly the current formants and level (no refit: refitting
    /// the start to a new pitch would step the filters and gain at once, a
    /// click) to `to` fitted at `f0`.
    fn glide_from_here(&mut self, to: &[Formant; 3], secs: f32, f0: f32) {
        self.from = self.cur;
        self.level_from = self.level;
        self.to = fit_to_pitch(to, f0);
        self.level_to = voiced_level(&self.to, f0, self.sr);
        self.glide = Crossfade::new(secs, self.sr);
        self.f0 = f0;
        self.then = None;
        self.active = true;
    }

    /// The formants right now.
    pub fn current(&self) -> [Formant; 3] {
        self.cur
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
                let f = Formant {
                    freq: a.freq + (b.freq - a.freq) * t,
                    q: a.q + (b.q - a.q) * t,
                    gain: a.gain + (b.gain - a.gain) * t,
                };
                self.filters[i].retune(f.freq, f.q, f.gain, self.sr);
                self.cur[i] = f;
            }
            self.level = self.level_from + (self.level_to - self.level_from) * t;
            if self.glide.done() {
                self.cur = self.to;
                if let Some((next, secs)) = self.then.take() {
                    let f0 = self.f0;
                    self.glide_from_here(&next, secs, f0);
                }
            }
        }
        let mut out = 0.0;
        for bq in self.filters.iter_mut() {
            out += bq.process(src);
        }
        out * self.level
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

/// The vowels English needs that the five cardinal ones miss, plus long-vowel
/// spellings, so a lyric can be written the way it sounds:
/// - `ee` / `oo` — the long vowels of "see" / "moon" (same as `i` / `u`, but
///   reads naturally in a respelled word);
/// - `ae` — the a in "cat" (without this, `ae` would glide a→e);
/// - a vowel then `h` — the short vowels: `ih` ("sit"), `uh` ("but"), `eh`
///   ("bed"); `ah`/`oh` are just `a`/`o`. The `h` is part of the vowel, not a
///   breath after it.
fn spelled_vowel(nucleus: &[char], lax: bool) -> Option<[Formant; 3]> {
    let f = |freq, q, gain| Formant { freq, q, gain };
    if lax && nucleus.len() == 1 {
        return match nucleus[0] {
            'i' => Some([f(400.0, 8.0, 1.0), f(1900.0, 10.0, 0.5), f(2550.0, 12.0, 0.3)]),
            'u' => Some([f(640.0, 8.0, 1.0), f(1190.0, 9.0, 0.6), f(2390.0, 12.0, 0.3)]),
            c => Some(vowel_formants(&c.to_string())),
        };
    }
    match nucleus {
        ['e', 'e'] => Some(vowel_formants("i")),
        ['o', 'o'] => Some(vowel_formants("u")),
        ['a', 'e'] => Some([f(660.0, 8.0, 1.0), f(1720.0, 10.0, 0.55), f(2410.0, 12.0, 0.3)]),
        _ => None,
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
    /// Where the mouth is closed or narrowed for this sound, as the F1-F3 a
    /// vowel slides out of (into) when it follows (precedes) it — the *locus*.
    /// That slide, not the noise, is most of what makes a p sound unlike a t or
    /// a k. All zero = no slide (h, whose shape is the vowel's own).
    pub locus: [f32; 3],
    /// Aspiration after the burst, seconds: breath through the vowel's shape
    /// before the voice comes in (the puff after p/t/k; all of an h).
    pub vot: f32,
    /// A stop's burst snaps on and decays; a fricative's hiss swells in.
    pub plosive: bool,
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

    /// Shorten the bursts (and their aspiration) in proportion so they last at
    /// most `max_secs` in all.
    /// A syllable sung (or babbled) in 70ms can't spend 90ms hissing an `s`.
    pub fn squeeze(&mut self, max_secs: f32) {
        let total: f32 = self.iter().map(|k| k.secs + k.vot).sum();
        if total > max_secs && total > 0.0 {
            let k = (max_secs / total).max(0.0);
            for b in self.bursts[..self.len as usize].iter_mut() {
                b.secs *= k;
                b.vot *= k;
            }
        }
    }

    pub fn last(&self) -> Option<&Consonant> {
        self.iter().last()
    }

    pub fn first(&self) -> Option<&Consonant> {
        self.iter().next()
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
    /// The vowel closes toward the first coda burst's locus before it ("cat":
    /// /a/ slides toward t as the voice stops), when there's no voiced coda.
    pub coda_locus: Option<[Formant; 3]>,
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
    let sh = Consonant {
        freq: 3000.0,
        q: 1.2,
        secs: 0.09,
        gain: 0.14,
        locus: PALATAL,
        vot: 0.0,
        plosive: false,
    };
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
            ('x', _) => {
                // "ks": a pop then a hiss.
                out.extend(consonant_for('k').map(Cons::Unvoiced));
                out.extend(consonant_for('s').map(Cons::Unvoiced));
                i += 1;
            }
            ('t', Some('h')) => {
                if let Some(mut k) = consonant_for('f') {
                    k.locus = DENTAL; // a soft fricative, between the teeth
                    out.push(Cons::Unvoiced(k));
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

/// Loci (F1, F2, F3 in Hz) by place of articulation. F1 is low for every
/// closure; F2 says where: low at the lips, mid behind the teeth, high where
/// the tongue humps up for k/sh.
pub const LABIAL: [f32; 3] = [250.0, 900.0, 2200.0];
pub const ALVEOLAR: [f32; 3] = [250.0, 1750.0, 2700.0];
pub const VELAR: [f32; 3] = [250.0, 1950.0, 2350.0];
pub const PALATAL: [f32; 3] = [250.0, 2100.0, 2800.0];
pub const DENTAL: [f32; 3] = [250.0, 1450.0, 2600.0];

/// Map a leading character to its consonant onset, if any.
fn consonant_for(c: char) -> Option<Consonant> {
    let cons = |freq, q, secs, gain, locus, vot, plosive| {
        Some(Consonant { freq, q, secs, gain, locus, vot, plosive })
    };
    match c {
        // A broad hiss (Q 1.3, not a narrow "pssh"), ~14 dB under the vowel.
        's' | 'z' => cons(5500.0, 1.3, 0.08, 0.12, ALVEOLAR, 0.0, false),
        'f' | 'v' => cons(4000.0, 1.0, 0.08, 0.146, LABIAL, 0.0, false), // broadband
        // h is breath through the vowel's own shape: no burst, all aspiration.
        'h' => cons(1500.0, 0.7, 0.003, 0.0, [0.0; 3], 0.07, false),
        't' | 'd' => cons(4000.0, 1.5, 0.012, 0.19, ALVEOLAR, 0.03, true), // sharp tick
        'k' | 'g' | 'c' | 'q' => cons(2000.0, 1.5, 0.015, 0.24, VELAR, 0.035, true), // mid pop
        'j' => cons(2800.0, 1.2, 0.05, 0.22, PALATAL, 0.01, false), // a short ch
        'p' | 'b' => cons(800.0, 1.2, 0.01, 0.235, LABIAL, 0.025, true), // low pop
        _ => None, // vowels + l/r/m/n/w/y
    }
}

/// A locus as formants, borrowing the vowel's sharpness and gains so only the
/// frequencies move during the slide. `None` for a consonant without one.
pub fn locus_formants(locus: &[f32; 3], vowel: &[Formant; 3]) -> Option<[Formant; 3]> {
    if locus[0] <= 0.0 {
        return None;
    }
    let mut out = *vowel;
    for (f, hz) in out.iter_mut().zip(locus.iter()) {
        f.freq = *hz;
    }
    Some(out)
}

/// Parse a sung syllable into onset / vowel / coda. Examples:
/// - `"sa"`   → unvoiced hiss onset, vowel /a/
/// - `"la"`   → voiced /l/ formants gliding into /a/
/// - `"ai"`   → diphthong: glide /a/ → /i/
/// - `"stra"` → onset cluster s+t bursts, /r/ glide, vowel /a/
/// - `"cat"`  → onset /k/, vowel /a/, coda /t/ burst
/// - `"sun"`  → onset /s/, vowel /u/, voiced /n/ coda glide
/// - `"sink"` → /s/, /i/, /n/ coda glide + /k/ burst
/// - `"luhv"` → /l/ glide, /ʌ/ (the `uh` spelling), /v/ burst
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

    // A spelled vowel (`ee`, `ae`, `uh`…) first; its trailing `h` belongs to it.
    let lax = coda_str.starts_with('h');
    let coda_str = if lax { &coda_str[1..] } else { &coda_str[..] };
    let spelled = spelled_vowel(&nucleus, lax);

    // Vowel nucleus, and a diphthong glide if the run holds two different vowels
    // (only when a voiced onset glide didn't already claim the glide).
    let last = nucleus.last().copied().unwrap_or('a');
    let vowel = spelled.unwrap_or_else(|| vowel_formants(&last.to_string()));
    if spelled.is_none() && glide_from.is_none() && nucleus.len() >= 2 && nucleus[0] != last {
        glide_from = Some(vowel_formants(&nucleus[0].to_string()));
        glide_secs = 0.15;
    }

    // An unvoiced onset with nothing voiced after it: the vowel slides out of
    // the last burst's locus ("ta" starts with its F2 up at t's 1750 Hz).
    if glide_from.is_none() {
        if let Some(k) = onset.last() {
            if let Some(lf) = locus_formants(&k.locus, &vowel) {
                glide_from = Some(lf);
                glide_secs = 0.045;
            }
        }
    }

    // Coda: an optional voiced ending to glide *to* (first voiced — a nasal/
    // liquid tail), then any unvoiced bursts (`t`, `s`, `k`…).
    let mut coda_glide = None;
    let mut coda = Cluster::default();
    for tok in parse_consonants(coda_str) {
        match tok {
            Cons::Voiced(vf) => {
                if coda_glide.is_none() {
                    coda_glide = Some(vf);
                }
            }
            Cons::Unvoiced(k) => coda.push(k),
        }
    }

    let coda_locus = match (coda_glide, coda.first()) {
        (None, Some(k)) => locus_formants(&k.locus, &vowel),
        _ => None,
    };

    SungSyllable {
        onset,
        glide_from,
        glide_secs,
        vowel,
        coda_glide,
        coda_locus,
        coda,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The spelled vowels land on their own formants: `uh` isn't `u`, `ae` is a
    /// vowel rather than an a→e glide, and the `h` doesn't become a coda breath.
    #[test]
    fn spelled_vowels_parse_as_vowels() {
        let uh = parse_syllable("buht");
        let u = parse_syllable("but");
        assert!(uh.vowel[0].freq > u.vowel[0].freq + 200.0);
        assert_eq!(uh.coda.len(), 1, "only the t is a coda burst");
        let ae = parse_syllable("aet");
        assert!(ae.glide_from.is_none(), "ae must not glide");
        assert!(ae.vowel[1].freq > 1600.0);
        let kae = parse_syllable("kaet");
        let from = kae.glide_from.expect("k's locus slides into the vowel");
        assert_eq!(from[1].freq, VELAR[1]);
        assert!(kae.coda_locus.is_some(), "the vowel closes toward the t");
        let ai = parse_syllable("mai");
        assert!(ai.glide_from.is_some(), "m still glides in");
        assert_eq!(parse_syllable("box").coda.len(), 2, "x is k+s");
    }
}

//! Reusable audio-effect primitives — small, stateful DSP blocks meant to drop
//! into a single voice now and (later) apply across a whole channel or the
//! master bus as the effects system grows.

use serde::{Deserialize, Serialize};

const TWO_PI: f32 = std::f32::consts::PI * 2.0;

/// A crossfade ramp from source A (t=0) to source B (t=1) over a fixed number of
/// samples. Advance one step per output sample via `mix`. Used now to blend a
/// consonant into its vowel; reusable later for a channel's A/B source swap or a
/// dry/wet effect fade.
#[derive(Clone, Copy)]
pub struct Crossfade {
    /// 0.0 = fully A, 1.0 = fully B.
    pos: f32,
    /// Advance per sample.
    step: f32,
}

impl Default for Crossfade {
    /// A finished crossfade (fully on B) — a no-op until `Crossfade::new` starts one.
    fn default() -> Self {
        Self {
            pos: 1.0,
            step: 0.0,
        }
    }
}

impl Crossfade {
    /// A crossfade that reaches B after `secs` seconds at `sample_rate`.
    pub fn new(secs: f32, sample_rate: f32) -> Self {
        let samples = (secs * sample_rate).max(1.0);
        Self {
            pos: 0.0,
            step: 1.0 / samples,
        }
    }

    /// True once fully faded to B.
    pub fn done(&self) -> bool {
        self.pos >= 1.0
    }

    /// Blend `a` (fading out) with `b` (fading in), advancing the ramp one sample.
    pub fn mix(&mut self, a: f32, b: f32) -> f32 {
        let t = self.pos.min(1.0);
        let out = a * (1.0 - t) + b * t;
        self.pos += self.step;
        out
    }
}

/// A feedback delay line — an **echo**. Owns a ring buffer sized to the delay
/// time; each sample it reads the delayed value, writes back `input + delayed *
/// feedback` (so echoes repeat and decay), and returns `input + delayed * mix`
/// (the dry signal plus the wet echoes). Inactive by default (a no-op that just
/// passes the input through) until `set` allocates the line. A channel-level
/// effect: it processes the channel's whole mixed output, so its echoes keep
/// ringing out after the notes stop (fed zero input, the tail decays by feedback).
#[derive(Default)]
pub struct Echo {
    buf: Vec<f32>,
    pos: usize,
    feedback: f32,
    /// Wet level: how loud the echoes are relative to the dry signal.
    mix: f32,
    active: bool,
}

impl Echo {
    /// Configure the delay: `secs` delay time, `feedback` (echo decay per repeat,
    /// clamped below 1 so it can't run away), `mix` (wet level). `secs <= 0`
    /// disables the effect and frees the buffer.
    pub fn set(&mut self, secs: f32, feedback: f32, mix: f32, sample_rate: f32) {
        let samples = (secs * sample_rate) as usize;
        if samples == 0 {
            self.active = false;
            self.buf = Vec::new();
            self.pos = 0;
            return;
        }
        // Resize (preserving as much tail as fits) and (re)configure.
        self.buf.resize(samples, 0.0);
        if self.pos >= self.buf.len() {
            self.pos = 0;
        }
        self.feedback = feedback.clamp(0.0, 0.95);
        self.mix = mix.max(0.0);
        self.active = true;
    }

    /// Clear the echo (silence the tail, keep it disabled). Used on app reload.
    pub fn clear(&mut self) {
        self.buf = Vec::new();
        self.pos = 0;
        self.feedback = 0.0;
        self.mix = 0.0;
        self.active = false;
    }

    /// Process one sample: returns dry + wet, advancing the delay line.
    pub fn process(&mut self, input: f32) -> f32 {
        if !self.active || self.buf.is_empty() {
            return input;
        }
        let delayed = self.buf[self.pos];
        self.buf[self.pos] = input + delayed * self.feedback;
        self.pos += 1;
        if self.pos >= self.buf.len() {
            self.pos = 0;
        }
        input + delayed * self.mix
    }
}

/// A biquad filter (transposed direct-form II) — the general second-order DSP
/// block. Coefficients are baked from (freq, q, sample_rate); `z1`/`z2` are the
/// per-instance state, so each independent stream needs its own. `retune` is
/// band-pass (constant 0 dB peak) with an output `gain`, used by the formant
/// bank; `retune_kind` selects low/high/band/notch (gain 1) for the channel
/// `Filter`. Retuning preserves state so a swept cutoff doesn't click. Default is
/// a silent/pass-nothing filter.
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

    /// Recompute band-pass coefficients in place, **preserving** filter state
    /// (z1/z2) so a continuous frequency glide/sweep doesn't click.
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

    /// Retune to one of the filter kinds (RBJ cookbook, unity gain), preserving
    /// state. `q` is resonance — ~0.707 is flat (Butterworth), higher peaks at
    /// the cutoff. Low/high shape the whole band; band/notch center on `freq`.
    pub fn retune_kind(&mut self, kind: FilterKind, freq: f32, q: f32, sample_rate: f32) {
        let w0 = TWO_PI * freq.clamp(10.0, sample_rate * 0.49) / sample_rate;
        let (sin_w0, cos_w0) = w0.sin_cos();
        let alpha = sin_w0 / (2.0 * q.max(0.001));
        let a0 = 1.0 + alpha;
        let (b0, b1, b2) = match kind {
            FilterKind::Low => {
                let k = 1.0 - cos_w0;
                (k * 0.5, k, k * 0.5)
            }
            FilterKind::High => {
                let k = 1.0 + cos_w0;
                (k * 0.5, -k, k * 0.5)
            }
            FilterKind::Band => (alpha, 0.0, -alpha),
            FilterKind::Notch => (1.0, -2.0 * cos_w0, 1.0),
        };
        self.b0 = b0 / a0;
        self.b1 = b1 / a0;
        self.b2 = b2 / a0;
        self.a1 = -2.0 * cos_w0 / a0;
        self.a2 = (1.0 - alpha) / a0;
        self.gain = 1.0;
    }

    /// Filter one input sample, advancing the state.
    pub fn process(&mut self, x: f32) -> f32 {
        let y = self.b0 * x + self.z1;
        self.z1 = self.b1 * x + self.z2 - self.a1 * y;
        self.z2 = self.b2 * x - self.a2 * y;
        y * self.gain
    }
}

/// The channel filter's response shape.
#[derive(Clone, Copy, PartialEq, Debug, Default, Serialize, Deserialize)]
pub enum FilterKind {
    /// Low-pass: keep below the cutoff (muffle/darken).
    #[default]
    Low,
    /// High-pass: keep above the cutoff (thin/brighten).
    High,
    /// Band-pass: keep a band around the cutoff.
    Band,
    /// Notch: cut a band around the cutoff.
    Notch,
}

/// A channel-level resonant filter with an optional cutoff **sweep**. Wraps a
/// `Biquad`; `set` picks the kind + cutoff + resonance, immediately or gliding
/// the cutoff over `secs` (retuning each sample, state preserved, so the sweep is
/// click-free). Inactive by default (passes input through) until `set`.
#[derive(Clone, Copy, Default)]
pub struct Filter {
    biquad: Biquad,
    kind: FilterKind,
    from_cutoff: f32,
    to_cutoff: f32,
    q: f32,
    sr: f32,
    sweep: Crossfade,
    active: bool,
}

impl Filter {
    /// Configure the filter. `secs <= 0` sets the cutoff immediately; otherwise
    /// the cutoff glides from its current value to `cutoff` over `secs` (a sweep).
    pub fn set(&mut self, kind: FilterKind, cutoff: f32, q: f32, secs: f32, sample_rate: f32) {
        self.kind = kind;
        self.q = q.max(0.001);
        self.sr = sample_rate;
        self.to_cutoff = cutoff.max(10.0);
        // Sweep from the current cutoff (or start there if it was inactive).
        self.from_cutoff = if self.active { self.from_cutoff } else { self.to_cutoff };
        if secs > 0.0 {
            self.sweep = Crossfade::new(secs, sample_rate);
        } else {
            self.from_cutoff = self.to_cutoff;
            self.sweep = Crossfade::default();
            self.biquad.retune_kind(kind, self.to_cutoff, self.q, sample_rate);
        }
        self.active = true;
    }

    /// Disable the filter (pass-through) and reset it. Used on app reload.
    pub fn clear(&mut self) {
        *self = Self::default();
    }

    /// Process one sample, advancing any in-flight cutoff sweep.
    pub fn process(&mut self, x: f32) -> f32 {
        if !self.active {
            return x;
        }
        if !self.sweep.done() {
            let t = self.sweep.mix(0.0, 1.0); // progress 0→1
            let cutoff = self.from_cutoff + (self.to_cutoff - self.from_cutoff) * t;
            self.biquad.retune_kind(self.kind, cutoff, self.q, self.sr);
            if self.sweep.done() {
                self.from_cutoff = self.to_cutoff; // settle for the next sweep
            }
        }
        self.biquad.process(x)
    }
}

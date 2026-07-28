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

/// One feedback comb filter with a one-pole low-pass in the loop (the damping) —
/// a Freeverb building block. Empty (silent) until sized by `Reverb::set`.
#[derive(Default)]
struct Comb {
    buf: Vec<f32>,
    pos: usize,
    /// Low-pass state in the feedback path (high-frequency damping).
    store: f32,
    feedback: f32,
    damp: f32,
}
impl Comb {
    fn process(&mut self, input: f32) -> f32 {
        if self.buf.is_empty() {
            return 0.0;
        }
        let out = self.buf[self.pos];
        self.store = out * (1.0 - self.damp) + self.store * self.damp;
        self.buf[self.pos] = input + self.store * self.feedback;
        self.pos += 1;
        if self.pos >= self.buf.len() {
            self.pos = 0;
        }
        out
    }
}

/// One Schroeder all-pass filter — smears the comb output so it reads as diffuse
/// reverb rather than discrete echoes. Empty (pass-through) until sized.
#[derive(Default)]
struct Allpass {
    buf: Vec<f32>,
    pos: usize,
    feedback: f32,
}
impl Allpass {
    fn process(&mut self, input: f32) -> f32 {
        if self.buf.is_empty() {
            return input;
        }
        let bufout = self.buf[self.pos];
        let out = -input + bufout;
        self.buf[self.pos] = input + bufout * self.feedback;
        self.pos += 1;
        if self.pos >= self.buf.len() {
            self.pos = 0;
        }
        out
    }
}

// Freeverb's tuned delay lengths (samples at 44.1 kHz), scaled to the device rate
// in `set`. The mutually-prime lengths are what make the tail sound smooth.
const COMB_TUNING: [usize; 8] = [1116, 1188, 1277, 1356, 1422, 1491, 1557, 1617];
const ALLPASS_TUNING: [usize; 4] = [556, 441, 341, 225];
const REVERB_TUNING_SR: f32 = 44100.0;

/// A **reverb** — a compact Freeverb (8 parallel damped comb filters summed, then
/// 4 series all-pass filters). A channel-level effect: it processes the channel's
/// whole mixed output and its tail rings out after the voices stop. Inactive by
/// default (passes input through) until `set` allocates the delay lines.
#[derive(Default)]
pub struct Reverb {
    combs: [Comb; 8],
    allpasses: [Allpass; 4],
    /// Wet level (how much reverb is mixed on top of the dry signal).
    wet: f32,
    active: bool,
}

impl Reverb {
    /// Configure the reverb. `room` is the tail length/decay (comb feedback,
    /// `0..1` — bigger = longer), `damp` rolls off the tail's highs (`0..1`),
    /// `wet` is the reverb level. `room <= 0` disables it and frees the buffers.
    pub fn set(&mut self, room: f32, damp: f32, wet: f32, sample_rate: f32) {
        if room <= 0.0 {
            self.clear();
            return;
        }
        let scale = sample_rate / REVERB_TUNING_SR;
        // Map room 0..1 to a safe feedback range (never ≥1, which would run away).
        let feedback = (0.7 + room.clamp(0.0, 1.0) * 0.28).min(0.98);
        let damp = damp.clamp(0.0, 1.0);
        for (comb, &tuning) in self.combs.iter_mut().zip(COMB_TUNING.iter()) {
            let len = ((tuning as f32 * scale) as usize).max(1);
            comb.buf.clear();
            comb.buf.resize(len, 0.0);
            comb.pos = 0;
            comb.store = 0.0;
            comb.feedback = feedback;
            comb.damp = damp;
        }
        for (ap, &tuning) in self.allpasses.iter_mut().zip(ALLPASS_TUNING.iter()) {
            let len = ((tuning as f32 * scale) as usize).max(1);
            ap.buf.clear();
            ap.buf.resize(len, 0.0);
            ap.pos = 0;
            ap.feedback = 0.5;
        }
        self.wet = wet.max(0.0);
        self.active = true;
    }

    /// Disable the reverb (pass-through) and free its buffers. Used on app reload.
    pub fn clear(&mut self) {
        *self = Self::default();
    }

    /// Process one sample: dry + wet reverb, advancing all the delay lines.
    pub fn process(&mut self, input: f32) -> f32 {
        if !self.active {
            return input;
        }
        // Fixed input gain (Freeverb) keeps the summed comb feedback bounded.
        let inp = input * 0.015;
        let mut out = 0.0;
        for comb in self.combs.iter_mut() {
            out += comb.process(inp);
        }
        for ap in self.allpasses.iter_mut() {
            out = ap.process(out);
        }
        input + out * self.wet
    }
}

/// A **bitcrusher** — the classic retro/lo-fi degrade, in two independent parts:
/// *bit-depth* reduction (quantize the amplitude to `2^bits` steps, adding the
/// gritty quantization noise of an 8-bit console) and *sample-rate* decimation
/// (latch the signal at a lower rate and hold it, adding aliasing sizzle).
/// Allocation-free (no delay lines), so it's safe to toggle per frame as a
/// momentary "punch-in" effect. Inactive by default (passes input through).
#[derive(Clone, Copy, Default)]
pub struct Crush {
    /// Quantization step; `0` = no bit reduction.
    step: f32,
    /// Latch rate as a fraction of the device rate (`>= 1` = every sample).
    rate_inc: f32,
    /// Latch phase accumulator and the currently held output value.
    phase: f32,
    held: f32,
    active: bool,
}

impl Crush {
    /// Configure the crusher. `bits` is the target bit depth (1 = harshest,
    /// `>= 16` = no quantization, `<= 0` disables the whole effect); `rate` is the
    /// target sample rate in Hz (`<= 0` or above the device rate = no decimation).
    pub fn set(&mut self, bits: f32, rate: f32, sample_rate: f32) {
        if bits <= 0.0 {
            *self = Self::default();
            return;
        }
        let bits = bits.min(16.0);
        // 2^bits levels across the -1..1 range. At 1 bit this is a 3-level
        // (-1/0/+1) square-ish crunch, which is the sound people want from it.
        self.step = if bits >= 16.0 {
            0.0
        } else {
            2.0 / 2f32.powf(bits)
        };
        self.rate_inc = if rate > 0.0 {
            (rate / sample_rate).min(1.0)
        } else {
            1.0
        };
        self.phase = 1.0; // latch immediately on the next sample
        self.active = true;
    }

    /// Disable the crusher (pass-through).
    pub fn clear(&mut self) {
        *self = Self::default();
    }

    /// Process one sample: decimate, then quantize.
    pub fn process(&mut self, x: f32) -> f32 {
        if !self.active {
            return x;
        }
        self.phase += self.rate_inc;
        if self.phase >= 1.0 {
            self.phase -= 1.0;
            self.held = if self.step > 0.0 {
                (x / self.step).round() * self.step
            } else {
                x
            };
        }
        self.held
    }
}

/// The waveshaping curve a `Drive` uses.
#[derive(Clone, Copy, PartialEq, Debug, Default, Serialize, Deserialize)]
pub enum DriveShape {
    /// Smooth tanh saturation — warm, tube-ish, level-compensated.
    #[default]
    Soft,
    /// Hard clipping — buzzy and aggressive, the classic square-off.
    Hard,
    /// Foldback — peaks reflect back down instead of clipping, for chaotic
    /// ring-mod-flavoured harmonics.
    Fold,
}

/// Reflect a value back into `-1..1` instead of clipping it (foldback distortion).
fn foldback(mut v: f32) -> f32 {
    let mut guard = 0;
    while v.abs() > 1.0 && guard < 8 {
        v = v.signum() * (2.0 - v.abs());
        guard += 1;
    }
    v.clamp(-1.0, 1.0)
}

/// **Drive / distortion** — a waveshaper: boost the signal into a nonlinear curve
/// so it saturates and grows harmonics. Allocation-free, so it's safe to toggle
/// per frame. Inactive by default (passes input through).
#[derive(Clone, Copy, Default)]
pub struct Drive {
    /// Gain into the curve, and the makeup gain after it.
    pre: f32,
    post: f32,
    shape: DriveShape,
    active: bool,
}

impl Drive {
    /// Configure the drive. `amount` is a normalized `0..1` knob (`<= 0` disables);
    /// `shape` picks the curve. Soft is level-compensated so raising the amount
    /// adds grit rather than just volume.
    pub fn set(&mut self, amount: f32, shape: DriveShape) {
        if amount <= 0.0 {
            *self = Self::default();
            return;
        }
        let a = amount.clamp(0.0, 1.0);
        self.pre = 1.0 + a * 24.0; // up to 25x into the curve
        self.post = match shape {
            // Normalize so a full-scale input still lands at full scale.
            DriveShape::Soft => 1.0 / self.pre.tanh(),
            // Already bounded to ±1 by the curve itself.
            DriveShape::Hard | DriveShape::Fold => 1.0,
        };
        self.shape = shape;
        self.active = true;
    }

    /// Disable the drive (pass-through).
    pub fn clear(&mut self) {
        *self = Self::default();
    }

    /// Process one sample through the waveshaper.
    pub fn process(&mut self, x: f32) -> f32 {
        if !self.active {
            return x;
        }
        let v = x * self.pre;
        let y = match self.shape {
            DriveShape::Soft => v.tanh(),
            DriveShape::Hard => v.clamp(-1.0, 1.0),
            DriveShape::Fold => foldback(v),
        };
        y * self.post
    }
}

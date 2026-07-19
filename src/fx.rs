//! Reusable audio-effect primitives — small, stateful DSP blocks meant to drop
//! into a single voice now and (later) apply across a whole channel or the
//! master bus as the effects system grows.

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

use std::{
    collections::VecDeque,
    sync::{
        mpsc::{channel, Receiver, Sender},
        Arc,
    },
};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use serde::{Deserialize, Serialize};
use rustc_hash::FxHashMap;

#[derive(Debug)]
struct Opt {
    #[cfg(all(
        any(target_os = "linux", target_os = "dragonfly", target_os = "freebsd"),
        feature = "jack"
    ))]
    jack: bool,

    device: String,
}

impl Opt {
    fn from_args() -> Self {
        #[cfg(all(
            any(target_os = "linux", target_os = "dragonfly", target_os = "freebsd"),
            feature = "jack"
        ))]
        let matches = app.get_matches();
        let device = ("default").to_string();

        #[cfg(all(
            any(target_os = "linux", target_os = "dragonfly", target_os = "freebsd"),
            feature = "jack"
        ))]
        return Opt {
            jack: matches.is_present("jack"),
            device,
        };

        #[cfg(any(
            not(any(target_os = "linux", target_os = "dragonfly", target_os = "freebsd")),
            not(feature = "jack")
        ))]
        Opt { device }
    }
}

pub fn init() -> (anyhow::Result<cpal::Stream>, Sender<SoundCommand>) {
    let (singer, audience) = channel::<SoundCommand>();
    (init_sound(audience), singer)
}

pub fn init_sound(audience: Receiver<SoundCommand>) -> anyhow::Result<cpal::Stream> {
    let opt = Opt::from_args();

    // Conditionally compile with jack if the feature is specified.
    #[cfg(all(
        any(target_os = "linux", target_os = "dragonfly", target_os = "freebsd"),
        feature = "jack"
    ))]
    let host = if opt.jack {
        cpal::host_from_id(cpal::available_hosts()
            .into_iter()
            .find(|id| *id == cpal::HostId::Jack)
            .expect(
                "make sure --features jack is specified. only works on OSes where jack is available",
            )).expect("jack host unavailable")
    } else {
        cpal::default_host()
    };

    #[cfg(any(
        not(any(target_os = "linux", target_os = "dragonfly", target_os = "freebsd")),
        not(feature = "jack")
    ))]
    let host = cpal::default_host();

    // Return an error rather than panicking when no device/config is available
    // (e.g. a browser with no/blocked audio): the caller (root.rs) logs it and
    // continues in silence instead of aborting the whole app.
    let device = if opt.device == "default" {
        host.default_output_device()
    } else {
        host.output_devices()?
            .find(|x| x.name().map(|y| y == opt.device).unwrap_or(false))
    }
    .ok_or_else(|| anyhow::anyhow!("no audio output device found"))?;
    println!("Output device: {}", device.name()?);

    let config = device.default_output_config()?;
    println!("Default output config: {:?}", config);

    let sample_format = config.sample_format();
    // Only the wasm block below mutates this; native uses the device default.
    #[allow(unused_mut)]
    let mut stream_config: cpal::StreamConfig = config.into();
    // Web audio runs its callback on the main thread (cpal's ScriptProcessor
    // backend), competing with rendering — a small buffer starves and crackles.
    // Ask for a larger buffer to ride through main-thread hitches. Costs latency
    // (~85ms at 48kHz) but kills the static. Native keeps the device default.
    #[cfg(target_arch = "wasm32")]
    {
        // cpal's WebAudio backend can report a default rate (e.g. 44100) that
        // differs from the browser's real AudioContext rate (often 48000 on
        // Mac hardware). Generating at the wrong rate makes the backend resample
        // every sample → "jumpy" audio. Probe the real rate and generate at it.
        if let Ok(ctx) = web_sys::AudioContext::new() {
            let real = ctx.sample_rate() as u32;
            let _ = ctx.close();
            log::info!(
                "audio: browser AudioContext rate {} Hz (cpal default {} Hz)",
                real,
                stream_config.sample_rate.0
            );
            if real > 0 {
                stream_config.sample_rate = cpal::SampleRate(real);
            }
        }
        stream_config.buffer_size = cpal::BufferSize::Fixed(4096);
    }
    log::info!(
        "audio: {} Hz, {} ch, buffer {:?}",
        stream_config.sample_rate.0,
        stream_config.channels,
        stream_config.buffer_size
    );

    match sample_format {
        cpal::SampleFormat::F32 => run::<f32>(&device, &stream_config, audience),
        cpal::SampleFormat::I16 => run::<i16>(&device, &stream_config, audience),
        cpal::SampleFormat::U16 => run::<u16>(&device, &stream_config, audience),
        // cpal 0.16 added many more sample formats; render f32 into whatever the
        // device wants where we can, else bail with a clear error.
        cpal::SampleFormat::I32 => run::<i32>(&device, &stream_config, audience),
        cpal::SampleFormat::U32 => run::<u32>(&device, &stream_config, audience),
        cpal::SampleFormat::F64 => run::<f64>(&device, &stream_config, audience),
        other => Err(anyhow::anyhow!("unsupported sample format {:?}", other)),
    }
}

/// Number of independent playback channels.
const NUM_CH: usize = 16;
const TWO_PI: f32 = std::f32::consts::PI * 2.0;

pub fn run<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    audience: Receiver<SoundCommand>,
) -> Result<cpal::Stream, anyhow::Error>
where
    T: cpal::SizedSample + cpal::FromSample<f32>,
{
    let sample_rate = config.sample_rate.0 as f32;
    let out_channels = config.channels as usize;
    println!("Sample rate: {}  out channels: {}", sample_rate, out_channels);

    // Per-instrument ADSR (see Envelope / Voice::advance_env) shapes each voice;
    // rates are derived per sample from the instrument's envelope + sample_rate.
    // Master gain feeding a soft limiter (see the tanh below). A low gain used
    // to be the only headroom against 16 voices clipping, but it also buried a
    // lone note/sample at ~20% — far quieter than the source file. With the
    // limiter we can run a healthy gain: single notes stay loud and ~linear,
    // dense polyphony compresses smoothly instead of hard-clipping.
    let master_volume = 0.5;

    // Default instrument for notes that don't name one: a plain square wave.
    let default_instr = Instrument::oscillator(usize::MAX, WaveType::Square);
    let mut instruments: FxHashMap<usize, Instrument> = FxHashMap::default();
    let mut samples: FxHashMap<usize, Sample> = FxHashMap::default();
    // Name->(PCM, source sample-rate) bank for sounds loaded from disk
    // (sounds/*.ogg). Consulted only when a `smpl(id, 'name')` binds a file into
    // an integer slot — never in the mixer, which stays purely index-keyed.
    // Buffers are Arc-shared.
    let mut loaded: FxHashMap<String, (Arc<[f32]>, f32)> = FxHashMap::default();

    // Per-channel state: `current` is the sounding voice, `fading` is a
    // just-released voice still ramping down so consecutive notes cross-fade
    // instead of clicking.
    let mut queues: [VecDeque<Note>; NUM_CH] = Default::default();
    let mut current: [Option<Voice>; NUM_CH] = Default::default();
    let mut fading: [Option<Voice>; NUM_CH] = Default::default();

    let mut steal_ch = 0usize;
    let mut next_value = move || -> f32 {
        // Drain every pending command each sample so triggers are effectively
        // sample-accurate (the old code polled once per ~2000 samples, which
        // quantised note timing and could drop/merge fast notes).
        while let Ok(cmd) = audience.try_recv() {
            match cmd {
                SoundCommand::PlayNote(note, ch) => {
                    let c = pick_channel(ch, &current, &fading, &queues, &mut steal_ch);
                    queues[c].push_back(note);
                }
                SoundCommand::Chain(notes, ch) => {
                    let c = pick_channel(ch, &current, &fading, &queues, &mut steal_ch);
                    queues[c].extend(notes);
                }
                SoundCommand::MakeInstrument(inst) => {
                    instruments.insert(inst.name, inst.normalized());
                }
                SoundCommand::MakeSample(id, mut pcm, base_freq, env) => {
                    let base_freq = if base_freq > 0.0 { base_freq } else { 440.0 };
                    // Loudness-match: samples come in at whatever amplitude the
                    // author happened to bake, so normalise every buffer to a
                    // consistent level (see normalize_pcm) — otherwise one sample
                    // is inaudible and the next is deafening.
                    normalize_pcm(&mut pcm);
                    // Raw Lua PCM has no source rate — treat it as device-rate
                    // (rate_ratio 1.0), preserving the old 1-sample-per-output step.
                    samples.insert(
                        id,
                        Sample {
                            pcm: pcm.into(),
                            base_freq,
                            rate_ratio: 1.0,
                        },
                    );
                    instruments.insert(id, Instrument::sample(id).with_env(env));
                }
                SoundCommand::LoadSample(name, mut pcm, source_rate) => {
                    // Boot: a sounds/*.ogg was decoded on the host. Stash it in
                    // the name bank (normalised, Arc-shared) with its source
                    // sample-rate; a later `smpl(id, name)` binds it into a slot.
                    normalize_pcm(&mut pcm);
                    let source_rate = if source_rate > 0.0 { source_rate } else { sample_rate };
                    loaded.insert(name, (pcm.into(), source_rate));
                }
                SoundCommand::BindSample(id, name, base, env) => {
                    // `smpl(id, 'name')`: resolve the loaded file once and copy
                    // its (Arc) buffer into integer slot `id`. Missing name =>
                    // leave the slot as-is (the note falls back to a default).
                    if let Some((pcm, source_rate)) = loaded.get(&name) {
                        samples.insert(
                            id,
                            Sample {
                                pcm: Arc::clone(pcm),
                                base_freq: base.filter(|b| *b > 0.0).unwrap_or(440.0),
                                rate_ratio: source_rate / sample_rate,
                            },
                        );
                        instruments.insert(id, Instrument::sample(id).with_env(env));
                    }
                }
                SoundCommand::Reset => {
                    // App (re)load: wipe user-defined instruments/samples, the
                    // loaded-file bank, and silence every voice so a removed
                    // `smpl`/`instr` (or a since-deleted sound file) can't linger.
                    instruments.clear();
                    samples.clear();
                    loaded.clear();
                    for c in 0..NUM_CH {
                        queues[c].clear();
                        current[c] = None;
                        fading[c] = None;
                    }
                }
                SoundCommand::Stop(ch) => match ch {
                    Some(c) => {
                        release_channel(c.min(NUM_CH - 1), &mut current, &mut fading, &mut queues)
                    }
                    None => {
                        for c in 0..NUM_CH {
                            release_channel(c, &mut current, &mut fading, &mut queues);
                        }
                    }
                },
                SoundCommand::FadeChannel(ch, _dur) => {
                    release_channel(ch.min(NUM_CH - 1), &mut current, &mut fading, &mut queues);
                }
            }
        }

        let mut mix = 0.0f32;
        for c in 0..NUM_CH {
            // Start the next queued note when the channel is idle.
            if current[c].is_none() {
                if let Some(note) = queues[c].pop_front() {
                    let inc = TWO_PI * note.frequency / sample_rate;
                    current[c] = Some(Voice::from_note(&note, inc));
                }
            }

            // Sounding voice: run the attack→decay→sustain envelope, then hand
            // off to `fading` for release at note-off.
            if let Some(v) = current[c].as_mut() {
                let instr = instruments.get(&v.instrument).unwrap_or(&default_instr);
                v.advance_env(&instr.env, sample_rate);
                mix += voice_out(v, instr, &samples) * v.volume * v.env;
                // One-shot samples play to their natural end, ignoring the note's
                // duration — otherwise a pitched-DOWN sample (slower playback)
                // gets cut off mid-buffer while still loud, an audible snap.
                // voice_out sets remaining=0 when the buffer runs out, so the
                // release still fires; oscillators use the duration timer.
                if !matches!(instr.wave, WaveType::Sample(_)) {
                    v.remaining -= 1.0 / sample_rate;
                }
                if v.remaining <= 0.0 {
                    // Note-off: move to release. Set the stage on the taken voice
                    // so we don't extend `v`'s borrow across the take.
                    if let Some(mut voice) = current[c].take() {
                        voice.stage = EnvStage::Release;
                        fading[c] = Some(voice);
                    }
                }
            }

            // Fading voice: release ramp to silence at the instrument's rate.
            if let Some(v) = fading[c].as_mut() {
                let instr = instruments.get(&v.instrument).unwrap_or(&default_instr);
                let rel = instr.env.release;
                v.env -= if rel > 0.0 { 1.0 / (rel * sample_rate) } else { 1.0 };
                if v.env <= 0.0 {
                    fading[c] = None;
                } else {
                    mix += voice_out(v, instr, &samples) * v.volume * v.env;
                }
            }
        }

        // Soft limiter: tanh is ~linear for small signals (a lone note passes
        // through almost untouched) and saturates gently toward ±1 as voices
        // stack up — no harsh hard-clip, and the output is always bounded.
        (mix * master_volume).tanh()
    };

    let err_fn = |err| eprintln!("audio stream error: {}", err);
    let stream = device.build_output_stream(
        config,
        move |data: &mut [T], _: &cpal::OutputCallbackInfo| {
            write_data(data, out_channels, &mut next_value)
        },
        err_fn,
        None,
    )?;
    stream.play()?;
    Ok(stream)
}

/// Additive synthesis: sum harmonics (index i => harmonic i+1) at the voice's
/// current phase, normalised by the amplitude sum so output stays in ~[-1, 1].
/// Oscillator shape for an instrument. `Additive` sums the harmonic table;
/// the rest are direct waveform generators (cheaper, punchier, more chip-like).
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum WaveType {
    Additive,
    Sine,
    Square,
    Saw,
    Triangle,
    Pulse(f32), // duty cycle 0..1 (0.5 == square)
    Noise,
    /// One-shot PCM sample, keyed into the sample store; played back pitched.
    Sample(usize),
}

/// A loaded PCM sample (mono, -1..1). `base_freq` is the pitch at which it plays
/// back 1:1 — a note above/below it resamples faster/slower. `rate_ratio` is the
/// sample's source sample-rate divided by the audio device's rate: it corrects
/// for a buffer recorded at, say, 44.1kHz playing on a 48kHz device, so pitch
/// and tempo stay accurate (raw Lua PCM has no source rate, so it's 1.0). The
/// buffer is an `Arc` so binding one loaded file into several slots shares it.
pub struct Sample {
    pcm: Arc<[f32]>,
    base_freq: f32,
    rate_ratio: f32,
}

/// Sample one instrument at `phase` (0..2π). `rng` is the voice's noise state.
fn osc(phase: f32, instr: &Instrument, rng: &mut u32) -> f32 {
    const PI: f32 = std::f32::consts::PI;
    match instr.wave {
        WaveType::Additive => {
            let mut total = 0.0;
            for (i, amp) in instr.freqs.iter().enumerate() {
                if *amp != 0.0 {
                    total += (phase * (i + 1) as f32).sin() * amp;
                }
            }
            total / instr.divisor
        }
        WaveType::Sine => phase.sin(),
        WaveType::Square => {
            if phase < PI {
                1.0
            } else {
                -1.0
            }
        }
        WaveType::Pulse(w) => {
            if phase < TWO_PI * w {
                1.0
            } else {
                -1.0
            }
        }
        WaveType::Saw => phase / PI - 1.0, // ramps -1..1 across the cycle
        WaveType::Triangle => {
            let t = phase / TWO_PI;
            if t < 0.5 {
                4.0 * t - 1.0
            } else {
                3.0 - 4.0 * t
            }
        }
        WaveType::Noise => {
            // xorshift-ish LCG per voice; full-rate white noise (ignores phase).
            *rng = rng.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            (*rng >> 8) as f32 / 8_388_607.5 - 1.0
        }
        // Samples are handled in voice_out (they need the store + a cursor).
        WaveType::Sample(_) => 0.0,
    }
}

/// One sample of a voice: advances its cursor and returns the raw signal.
/// Sample instruments resample their PCM by pitch (freq/base_freq) with linear
/// interpolation; everything else advances phase and runs `osc`.
fn voice_out(v: &mut Voice, instr: &Instrument, samples: &FxHashMap<usize, Sample>) -> f32 {
    if let WaveType::Sample(sid) = instr.wave {
        if let Some(s) = samples.get(&sid) {
            let i = v.sample_pos as usize;
            let out = if i + 1 < s.pcm.len() {
                let frac = v.sample_pos - i as f32;
                s.pcm[i] * (1.0 - frac) + s.pcm[i + 1] * frac
            } else if i < s.pcm.len() {
                s.pcm[i]
            } else {
                v.remaining = 0.0; // one-shot finished => release
                0.0
            };
            // Cursor step = pitch ratio × source/device rate ratio, so a note at
            // base_freq plays the buffer at its recorded speed regardless of the
            // device rate (fixes ~8% pitch/tempo drift on a 48kHz device).
            v.sample_pos += (v.freq / s.base_freq * s.rate_ratio).max(0.0);
            out
        } else {
            0.0
        }
    } else {
        v.advance();
        osc(v.phase, instr, &mut v.rng)
    }
}

fn write_data<T>(output: &mut [T], out_channels: usize, next_sample: &mut dyn FnMut() -> f32)
where
    T: cpal::Sample + cpal::FromSample<f32>,
{
    for frame in output.chunks_mut(out_channels) {
        let value: T = T::from_sample(next_sample());
        for sample in frame.iter_mut() {
            *sample = value;
        }
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Note {
    instrument: usize,
    pub frequency: f32,
    /// Sustain length in seconds (before the release ramp).
    pub duration: f32,
    pub volume: f32,
}
impl Note {
    pub fn new(instrument: usize, frequency: f32, duration: f32, volume: f32) -> Self {
        Self {
            instrument,
            frequency,
            duration,
            volume,
        }
    }
}

/// Per-instrument ADSR envelope. Times are in seconds; `sustain` is a level
/// (0..1) held while the note sounds. Defaults reproduce the old fixed envelope
/// (fast attack, no decay, full sustain, short release) so untouched instruments
/// sound exactly as before.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Envelope {
    pub attack: f32,
    pub decay: f32,
    pub sustain: f32,
    pub release: f32,
}
impl Default for Envelope {
    fn default() -> Self {
        Self {
            attack: 0.004,
            decay: 0.0,
            sustain: 1.0,
            release: 0.012,
        }
    }
}

/// Which segment of the ADSR a voice is currently in.
#[derive(Clone, Copy, PartialEq)]
enum EnvStage {
    Attack,
    Decay,
    Sustain,
    Release,
}

/// Runtime state for one sounding note: a wrapped phase accumulator plus an ADSR
/// envelope. Kept separate from `Note` (the queued spec).
struct Voice {
    phase: f32,
    phase_inc: f32,
    instrument: usize,
    remaining: f32,
    volume: f32,
    env: f32,
    stage: EnvStage,
    /// Per-voice noise RNG state (seeded from the note so it varies per voice).
    rng: u32,
    /// Playback cursor (float index) for Sample instruments.
    sample_pos: f32,
    /// The note's frequency, kept so Sample voices can resample by pitch.
    freq: f32,
}
impl Voice {
    fn from_note(note: &Note, phase_inc: f32) -> Self {
        Self {
            phase: 0.0,
            phase_inc,
            instrument: note.instrument,
            remaining: note.duration.max(0.0),
            volume: note.volume,
            env: 0.0,
            stage: EnvStage::Attack,
            rng: note.frequency.to_bits() | 1, // nonzero, varies per note
            sample_pos: 0.0,
            freq: note.frequency,
        }
    }
    /// Advance and wrap the phase to keep f32 precision indefinitely.
    fn advance(&mut self) {
        self.phase += self.phase_inc;
        if self.phase >= TWO_PI {
            self.phase -= TWO_PI;
        }
    }
    /// Step the attack→decay→sustain portion of the envelope one sample. Release
    /// is driven separately (in the fading block) after note-off.
    fn advance_env(&mut self, env: &Envelope, sr: f32) {
        match self.stage {
            EnvStage::Attack => {
                self.env += if env.attack > 0.0 { 1.0 / (env.attack * sr) } else { 1.0 };
                if self.env >= 1.0 {
                    self.env = 1.0;
                    self.stage = EnvStage::Decay;
                }
            }
            EnvStage::Decay => {
                if env.decay > 0.0 && env.sustain < 1.0 {
                    self.env -= (1.0 - env.sustain) / (env.decay * sr);
                    if self.env <= env.sustain {
                        self.env = env.sustain;
                        self.stage = EnvStage::Sustain;
                    }
                } else {
                    self.env = env.sustain;
                    self.stage = EnvStage::Sustain;
                }
            }
            EnvStage::Sustain => self.env = env.sustain,
            EnvStage::Release => {} // handled after note-off
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Instrument {
    name: usize,
    wave: WaveType,
    /// Harmonic amplitudes (Additive only); index i is harmonic (i+1).
    freqs: Vec<f32>,
    divisor: f32,
    env: Envelope,
}

impl Instrument {
    /// Additive instrument from a harmonic-amplitude table.
    pub fn additive(name: usize, freqs: Vec<f32>) -> Self {
        let divisor = Self::compute_divisor(&freqs);
        Self {
            name,
            wave: WaveType::Additive,
            freqs,
            divisor,
            env: Envelope::default(),
        }
    }
    /// Direct-waveform instrument (square/saw/triangle/pulse/noise/sine).
    pub fn oscillator(name: usize, wave: WaveType) -> Self {
        Self {
            name,
            wave,
            freqs: Vec::new(),
            divisor: 1.0,
            env: Envelope::default(),
        }
    }
    /// Sample-playback instrument, keyed to a stored sample of the same id.
    pub fn sample(name: usize) -> Self {
        Self {
            name,
            wave: WaveType::Sample(name),
            freqs: Vec::new(),
            divisor: 1.0,
            env: Envelope::default(),
        }
    }
    /// Set the envelope (builder style, used by the command layer).
    pub fn with_env(mut self, env: Envelope) -> Self {
        self.env = env;
        self
    }
    fn compute_divisor(freqs: &[f32]) -> f32 {
        let sum: f32 = freqs.iter().map(|a| a.abs()).sum();
        if sum > 0.0 {
            sum
        } else {
            1.0
        }
    }
    /// Recompute the normalisation divisor (after freqs are set by the caller).
    fn normalized(mut self) -> Self {
        self.divisor = Self::compute_divisor(&self.freqs);
        self
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub enum SoundCommand {
    MakeInstrument(Instrument),
    /// Store a PCM sample (id, mono samples -1..1, base pitch, envelope) and
    /// register a same-id instrument that plays it.
    MakeSample(usize, Vec<f32>, f32, Envelope),
    /// Stash a disk-loaded sound in the name bank (name, mono PCM, source
    /// sample-rate). Sent by the asset loader at boot; bound later by BindSample.
    LoadSample(String, Vec<f32>, f32),
    /// Bind a name-banked sound into integer instrument slot `id`, optionally
    /// overriding its base pitch, with an envelope. Backs `smpl(id, 'name', cfg?)`.
    BindSample(usize, String, Option<f32>, Envelope),
    /// Play one note. `Some(ch)` targets a channel; `None` auto-allocates a free
    /// one (so overlapping `note()` calls form chords).
    PlayNote(Note, Option<usize>),
    /// Queue a sequence on a single channel (`None` auto-allocates one).
    Chain(Vec<Note>, Option<usize>),
    /// Release a channel (`None` = all channels).
    Stop(Option<usize>),
    FadeChannel(usize, f32),
    /// Drop all user instruments/samples and silence every voice (app reload).
    Reset,
}

/// Loudness-match a sample buffer in place. Samples arrive at arbitrary
/// amplitude, so we scale toward a target RMS (perceived loudness) while
/// clamping the gain so the peak can't clip. Decaying one-shots (mostly quiet)
/// end up peak-limited — the loudest safe level — while sustained buffers hit
/// the RMS target. The result is comparable in loudness to the oscillators,
/// which run near full scale.
fn normalize_pcm(pcm: &mut [f32]) {
    let mut peak = 0.0f32;
    let mut sum_sq = 0.0f64;
    for &s in pcm.iter() {
        peak = peak.max(s.abs());
        sum_sq += (s as f64) * (s as f64);
    }
    if peak <= 0.0 {
        return; // silent buffer, nothing to scale
    }
    let rms = (sum_sq / pcm.len() as f64).sqrt() as f32;
    const TARGET_RMS: f32 = 0.4; // ~ a mid-loud oscillator
    const PEAK_CEIL: f32 = 0.98; // headroom against clipping
    let gain = if rms > 0.0 {
        (TARGET_RMS / rms).min(PEAK_CEIL / peak)
    } else {
        PEAK_CEIL / peak
    };
    for s in pcm.iter_mut() {
        *s *= gain;
    }
}

/// Pick the channel for a note: an explicit one, or the first fully-idle channel
/// so simultaneous notes voice separately. Falls back to round-robin stealing
/// when all 16 are busy (a 16-voice cap).
fn pick_channel(
    ch: Option<usize>,
    current: &[Option<Voice>],
    fading: &[Option<Voice>],
    queues: &[VecDeque<Note>],
    steal: &mut usize,
) -> usize {
    match ch {
        Some(c) => c.min(NUM_CH - 1),
        None => (0..NUM_CH)
            .find(|&c| current[c].is_none() && fading[c].is_none() && queues[c].is_empty())
            .unwrap_or_else(|| {
                let c = *steal;
                *steal = (*steal + 1) % NUM_CH;
                c
            }),
    }
}

/// Clear a channel's queue and release its sounding voice (ramp down, no click).
fn release_channel(
    c: usize,
    current: &mut [Option<Voice>],
    fading: &mut [Option<Voice>],
    queues: &mut [VecDeque<Note>],
) {
    queues[c].clear();
    if let Some(v) = current[c].take() {
        fading[c] = Some(v);
    }
}

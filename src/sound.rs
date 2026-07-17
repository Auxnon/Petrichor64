use std::{
    collections::VecDeque,
    sync::{
        mpsc::{channel, Receiver, Sender},
        Arc,
    },
};

#[cfg(not(target_arch = "wasm32"))]
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use serde::{Deserialize, Serialize};
use rustc_hash::FxHashMap;

use crate::fx::Crossfade;
use crate::vocaloid::{Biquad, Consonant, Formant, FormantBank};

#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug)]
struct Opt {
    #[cfg(all(
        any(target_os = "linux", target_os = "dragonfly", target_os = "freebsd"),
        feature = "jack"
    ))]
    jack: bool,

    device: String,
}

#[cfg(not(target_arch = "wasm32"))]
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

#[cfg(not(target_arch = "wasm32"))]
pub fn init() -> (anyhow::Result<cpal::Stream>, Sender<SoundCommand>) {
    let (singer, audience) = channel::<SoundCommand>();
    (init_sound(audience), singer)
}

#[cfg(not(target_arch = "wasm32"))]
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

/// Build the per-sample mixer over its own synth state, returned as a closure.
/// Shared by the native cpal stream and the wasm scheduled-buffer output — both
/// just pull f32 samples from it. Drains pending SoundCommands each call, so
/// note triggers stay effectively sample-accurate.
fn make_mixer(sample_rate: f32, audience: Receiver<SoundCommand>) -> impl FnMut() -> f32 {
    // Master gain feeding a soft limiter (the tanh below): single notes stay
    // loud and ~linear, dense polyphony compresses instead of hard-clipping.
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

    // Per-channel output gain + an optional in-flight fade (a Crossfade ramp).
    // The first channel-level "effect" — fade a channel in/out, or crossfade two
    // channels with a pair of opposite fades. Gain multiplies the channel's mix.
    let mut channel_gain: [f32; NUM_CH] = [1.0; NUM_CH];
    let mut channel_fade: [Crossfade; NUM_CH] = [Crossfade::default(); NUM_CH];
    let mut fade_from: [f32; NUM_CH] = [1.0; NUM_CH];
    let mut fade_to: [f32; NUM_CH] = [1.0; NUM_CH];

    // Current singing-voice character (breath + vibrato), applied to each new
    // sung note. Set by `vox`; reset on load.
    let mut voice_breath = 0.0f32;
    let mut voice_vib_depth = 0.0f32;
    let mut voice_vib_rate = 5.5f32;

    let mut steal_ch = 0usize;
    move || -> f32 {
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
                        channel_gain[c] = 1.0;
                        channel_fade[c] = Crossfade::default();
                    }
                    voice_breath = 0.0;
                    voice_vib_depth = 0.0;
                    voice_vib_rate = 5.5;
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
                SoundCommand::VoiceConfig(breath, depth, rate) => {
                    voice_breath = breath;
                    voice_vib_depth = depth;
                    voice_vib_rate = if rate > 0.0 { rate } else { 5.5 };
                }
                SoundCommand::FadeChannel(ch, secs, target) => {
                    // Ramp a channel's output gain to `target` over `secs` via a
                    // Crossfade. Fade out (target 0), fade in (1), or crossfade
                    // two channels with a pair of opposite fades.
                    let c = ch.min(NUM_CH - 1);
                    fade_from[c] = channel_gain[c];
                    fade_to[c] = target.clamp(0.0, 1.0);
                    channel_fade[c] = Crossfade::new(secs.max(0.0), sample_rate);
                }
            }
        }

        let mut mix = 0.0f32;
        for c in 0..NUM_CH {
            let mut ch_out = 0.0f32;
            // Start the next queued note when the channel is idle.
            if current[c].is_none() {
                if let Some(note) = queues[c].pop_front() {
                    let inc = TWO_PI * note.frequency / sample_rate;
                    let mut voice = Voice::from_note(&note, inc);
                    // Sung note: set the vowel formants, or glide from a voiced
                    // consonant / diphthong start into them; apply voice character.
                    if let Some(target) = &note.formants {
                        if let Some(from) = &note.glide_from {
                            voice
                                .voice_bank
                                .glide_to(from, target, note.glide_secs, sample_rate);
                        } else {
                            voice.voice_bank.set(target, sample_rate);
                        }
                        voice.breath = voice_breath;
                        voice.vib_depth = voice_vib_depth;
                        voice.vib_rate = voice_vib_rate;
                    }
                    // Consonant onset: a burst of band-passed noise, whose tail
                    // crossfades into the vowel (up to 12ms, at most half the
                    // onset) so it isn't an abrupt cut.
                    if let Some(k) = note.consonant {
                        let xfade_secs = (k.secs * 0.5).min(0.012);
                        let total = (k.secs * sample_rate) as u32;
                        let xfade_samps = (xfade_secs * sample_rate) as u32;
                        voice.consonant_samples = total.saturating_sub(xfade_samps);
                        voice.consonant_xfade = Crossfade::new(xfade_secs, sample_rate);
                        voice.consonant_filter = Biquad::bandpass(k.freq, k.q, k.gain, sample_rate);
                    }
                    current[c] = Some(voice);
                }
            }

            // Sounding voice: run the attack→decay→sustain envelope, then hand
            // off to `fading` for release at note-off.
            if let Some(v) = current[c].as_mut() {
                let instr = instruments.get(&v.instrument).unwrap_or(&default_instr);
                v.advance_env(&instr.env, sample_rate);
                mix += voice_out(v, instr, &samples, sample_rate) * v.volume * v.env;
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
                    ch_out += voice_out(v, instr, &samples, sample_rate) * v.volume * v.env;
                }
            }

            // Channel effect: advance an in-flight fade, then apply the gain.
            if !channel_fade[c].done() {
                channel_gain[c] = channel_fade[c].mix(fade_from[c], fade_to[c]);
            }
            mix += ch_out * channel_gain[c];
        }

        // Soft limiter: tanh is ~linear for small signals (a lone note passes
        // through almost untouched) and saturates gently toward ±1 as voices
        // stack up — no harsh hard-clip, and the output is always bounded.
        (mix * master_volume).tanh()
    }
}

/// Native audio output: a cpal stream pulling from the shared mixer.
#[cfg(not(target_arch = "wasm32"))]
pub fn run<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    audience: Receiver<SoundCommand>,
) -> Result<cpal::Stream, anyhow::Error>
where
    T: cpal::SizedSample + cpal::FromSample<f32>,
{
    let out_channels = config.channels as usize;
    let mut next_value = make_mixer(config.sample_rate.0 as f32, audience);
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

/// Web audio output: schedule short PCM chunks onto the AudioContext's own clock
/// ("two clocks" technique), so playback is glitch-free through main-thread jank
/// as long as we keep `lookahead` seconds queued. cpal's WebAudio backend ran on
/// the main thread and crackled; this bypasses it entirely.
#[cfg(target_arch = "wasm32")]
pub struct WebAudioOut {
    ctx: web_sys::AudioContext,
    mixer: Box<dyn FnMut() -> f32>,
    /// AudioContext time (seconds) at which the next chunk should start.
    next_time: f64,
    chunk_frames: usize,
    /// How far ahead of `currentTime` to keep audio queued. Bigger = smoother
    /// but laggier; this is the responsiveness dial.
    lookahead: f64,
    sr: f32,
    scratch: Vec<f32>,
}

#[cfg(target_arch = "wasm32")]
impl WebAudioOut {
    pub fn new(audience: Receiver<SoundCommand>) -> Result<Self, wasm_bindgen::JsValue> {
        let ctx = web_sys::AudioContext::new()?;
        let sr = ctx.sample_rate();
        let chunk_frames = (sr * 0.03).round().max(64.0) as usize; // ~30ms chunks
        log::info!("web audio: {} Hz, {}-frame chunks", sr, chunk_frames);
        Ok(Self {
            mixer: Box::new(make_mixer(sr, audience)),
            ctx,
            next_time: 0.0,
            chunk_frames,
            lookahead: 0.09,
            sr,
            scratch: vec![0.0; chunk_frames],
        })
    }

    /// A clone of the AudioContext (a JS reference) so a gesture handler can
    /// resume it — browsers start it suspended until the first user interaction.
    pub fn context(&self) -> web_sys::AudioContext {
        self.ctx.clone()
    }

    /// Generate and schedule chunks until we're `lookahead` ahead of the audio
    /// clock. Call once per frame. Falling behind (backgrounded tab) resyncs to
    /// `now` rather than dumping a backlog.
    pub fn pump(&mut self) {
        // Don't schedule into a suspended context (before the first user
        // gesture) — the browser rejects start() and spams the console. The
        // gesture handler resumes it; until then, stay quiet.
        if self.ctx.state() != web_sys::AudioContextState::Running {
            return;
        }
        let now = self.ctx.current_time();
        if self.next_time < now {
            self.next_time = now;
        }
        let chunk_secs = self.chunk_frames as f64 / self.sr as f64;
        while self.next_time < now + self.lookahead {
            for s in self.scratch.iter_mut() {
                *s = (self.mixer)();
            }
            let buf = match self.ctx.create_buffer(1, self.chunk_frames as u32, self.sr) {
                Ok(b) => b,
                Err(_) => break,
            };
            if buf.copy_to_channel(&mut self.scratch, 0).is_err() {
                break;
            }
            if let Ok(src) = self.ctx.create_buffer_source() {
                src.set_buffer(Some(&buf));
                let _ = src.connect_with_audio_node(&self.ctx.destination());
                let _ = src.start_with_when(self.next_time);
            }
            self.next_time += chunk_secs;
        }
    }
}

/// Create the web audio driver + the command sender the engine sends notes on.
#[cfg(target_arch = "wasm32")]
pub fn init_web() -> (Result<WebAudioOut, wasm_bindgen::JsValue>, Sender<SoundCommand>) {
    let (singer, audience) = channel::<SoundCommand>();
    (WebAudioOut::new(audience), singer)
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

/// Full-rate white noise from a per-voice xorshift-ish LCG (advances `rng`).
/// Shared by the noise oscillator and the consonant onsets.
fn white_noise(rng: &mut u32) -> f32 {
    *rng = rng.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
    (*rng >> 8) as f32 / 8_388_607.5 - 1.0
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
        WaveType::Noise => white_noise(rng),
        // Samples are handled in voice_out (they need the store + a cursor).
        WaveType::Sample(_) => 0.0,
    }
}

/// One sample of a voice: advances its cursor and returns the raw signal.
/// Sample instruments resample their PCM by pitch (freq/base_freq) with linear
/// interpolation; everything else advances phase and runs `osc`.
fn voice_out(v: &mut Voice, instr: &Instrument, samples: &FxHashMap<usize, Sample>, sr: f32) -> f32 {
    if let WaveType::Sample(sid) = instr.wave {
        return if let Some(s) = samples.get(&sid) {
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
        };
    }

    // Vibrato (sung voices): a slow LFO wobbles the pitch. Recompute phase_inc
    // from the modulated frequency before advancing.
    if v.voice_bank.is_active() && v.vib_depth > 0.0 {
        v.vib_phase += TWO_PI * v.vib_rate / sr;
        if v.vib_phase >= TWO_PI {
            v.vib_phase -= TWO_PI;
        }
        v.phase_inc = TWO_PI * v.freq * (1.0 + v.vib_depth * v.vib_phase.sin()) / sr;
    }

    // The tonal body of this voice: a sung vowel (sawtooth glottal source through
    // the formant bank) or a plain oscillator. Computed every sample so the
    // formant filters warm up during a consonant onset and are ready for the
    // crossfade. `advance` steps the phase.
    v.advance();
    let body = if v.voice_bank.is_active() {
        const PI: f32 = std::f32::consts::PI;
        // Breath: aspiration noise mixed into the glottal source, then shaped by
        // the same formants — a breathy vowel rather than a hiss on top.
        let saw = v.phase / PI - 1.0;
        let src = saw + v.breath * white_noise(&mut v.rng);
        v.voice_bank.process(src)
    } else {
        osc(v.phase, instr, &mut v.rng)
    };

    if v.consonant_samples > 0 {
        // Pure consonant: band-passed noise burst before the vowel.
        v.consonant_samples -= 1;
        v.consonant_filter.process(white_noise(&mut v.rng))
    } else if !v.consonant_xfade.done() {
        // Onset tail: crossfade the consonant noise out and the vowel body in.
        let cons = v.consonant_filter.process(white_noise(&mut v.rng));
        v.consonant_xfade.mix(cons, body)
    } else {
        body
    }
}

#[cfg(not(target_arch = "wasm32"))]
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
    /// A sung note: the vowel's three formants. `None` = a normal instrument
    /// note. When set, the voice uses a sawtooth glottal source shaped by these
    /// formants (see `vocaloid`), ignoring the instrument's waveform.
    pub formants: Option<[Formant; 3]>,
    /// Optional consonant onset (a short noise burst before the vowel).
    pub consonant: Option<Consonant>,
    /// Optional starting formants to glide *from* into `formants` (a voiced
    /// consonant, or a diphthong's first vowel), over `glide_secs`.
    pub glide_from: Option<[Formant; 3]>,
    pub glide_secs: f32,
}
impl Note {
    pub fn new(instrument: usize, frequency: f32, duration: f32, volume: f32) -> Self {
        Self {
            instrument,
            frequency,
            duration,
            volume,
            formants: None,
            consonant: None,
            glide_from: None,
            glide_secs: 0.0,
        }
    }
    /// A sung note at `frequency` from a parsed syllable (vowel + optional
    /// consonant onset + optional formant glide).
    pub fn sung(
        frequency: f32,
        duration: f32,
        volume: f32,
        syllable: &crate::vocaloid::SungSyllable,
    ) -> Self {
        Self {
            instrument: 0,
            frequency,
            duration,
            volume,
            formants: Some(syllable.vowel),
            consonant: syllable.consonant,
            glide_from: syllable.glide_from,
            glide_secs: syllable.glide_secs,
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
    /// Formant filters for a sung (voice) note; inactive for normal notes.
    voice_bank: FormantBank,
    /// Consonant onset: remaining samples of band-passed noise before the vowel.
    consonant_samples: u32,
    consonant_filter: Biquad,
    /// Blends the consonant noise into the vowel at the end of the onset.
    consonant_xfade: Crossfade,
    /// Voice character (sung notes only): breath (aspiration noise mix) and a
    /// vibrato LFO (depth as a pitch fraction, rate in Hz, running phase).
    breath: f32,
    vib_depth: f32,
    vib_rate: f32,
    vib_phase: f32,
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
            voice_bank: FormantBank::default(),
            consonant_samples: 0,
            consonant_filter: Biquad::default(),
            consonant_xfade: Crossfade::default(),
            breath: 0.0,
            vib_depth: 0.0,
            vib_rate: 0.0,
            vib_phase: 0.0,
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
    /// Ramp a channel's output gain: (channel, seconds, target gain 0..1).
    FadeChannel(usize, f32, f32),
    /// Set the singing-voice character for later sung notes: (breath, vibrato
    /// depth as a pitch fraction, vibrato rate Hz).
    VoiceConfig(f32, f32, f32),
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

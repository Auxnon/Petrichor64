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

use crate::fx::{Biquad, Crossfade, Crush, Drive, DriveShape, Echo, Filter, FilterKind, Reverb};
use crate::vocaloid::{Consonant, Formant, FormantBank};

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

/// Number of independent playback channels ("tracks").
const NUM_CH: usize = 16;
/// Default polyphony lanes per channel — how many notes one channel can sound at
/// once (chords). Generous by default so casual chords "just work" with no
/// config; carve specific channels up/down with `attr{ lanes = {…} }`.
const DEFAULT_LANES: usize = 8;
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

    // The channels ("tracks"). Each owns a pool of polyphony lanes plus its own
    // gain/fade and singing-voice character — a chord sounds across one channel's
    // lanes, so its character/effects stay consistent. `attr{lanes}` resizes them.
    // Built here, on the setup thread before the stream starts, so every
    // buffer-owning effect allocates its delay lines off the audio thread.
    let mut channels: Vec<Channel> = (0..NUM_CH)
        .map(|_| Channel::new(DEFAULT_LANES, sample_rate))
        .collect();

    move || -> f32 {
        // Drain every pending command each sample so triggers are effectively
        // sample-accurate (the old code polled once per ~2000 samples, which
        // quantised note timing and could drop/merge fast notes).
        while let Ok(cmd) = audience.try_recv() {
            match cmd {
                SoundCommand::PlayNote(note, ch) => {
                    let c = ch.unwrap_or(0).min(channels.len() - 1);
                    let l = channels[c].pick_lane();
                    channels[c].queues[l].push_back(note);
                }
                SoundCommand::Chain(notes, ch) => {
                    // A song/phrase sequences on a single lane of the channel.
                    let c = ch.unwrap_or(0).min(channels.len() - 1);
                    let l = channels[c].pick_lane();
                    channels[c].queues[l].extend(notes);
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
                    for chan in channels.iter_mut() {
                        chan.reset();
                    }
                }
                SoundCommand::Stop(ch) => match ch {
                    Some(c) => {
                        let c = c.min(channels.len() - 1);
                        channels[c].release_all();
                    }
                    None => {
                        for chan in channels.iter_mut() {
                            chan.release_all();
                        }
                    }
                },
                SoundCommand::ReleaseNote(ch, freq) => {
                    // Note-off: end the sustain of the matching voice so the
                    // normal note-off path runs (coda, then the release ramp).
                    // Frequencies come from the same note→Hz formula on both
                    // ends, so an epsilon compare is plenty.
                    let range = match ch {
                        Some(c) => {
                            let c = c.min(channels.len() - 1);
                            c..c + 1
                        }
                        None => 0..channels.len(),
                    };
                    'release: for c in range {
                        for l in 0..channels[c].lanes() {
                            if let Some(v) = channels[c].current[l].as_mut() {
                                if (v.freq - freq).abs() < 0.01 && v.remaining > 0.0 {
                                    v.remaining = 0.0;
                                    break 'release;
                                }
                            }
                        }
                    }
                }
                SoundCommand::VoiceConfig(ch, breath, depth, rate) => {
                    let c = ch.min(channels.len() - 1);
                    let chan = &mut channels[c];
                    chan.breath = breath;
                    chan.vib_depth = depth;
                    chan.vib_rate = if rate > 0.0 { rate } else { 5.5 };
                }
                SoundCommand::FadeChannel(ch, secs, target) => {
                    // Ramp a channel's output gain to `target` over `secs` via a
                    // Crossfade. Fade out (target 0), fade in (1), or crossfade
                    // two channels with a pair of opposite fades.
                    let c = ch.min(channels.len() - 1);
                    let chan = &mut channels[c];
                    chan.fade_from = chan.gain;
                    chan.fade_to = target.clamp(0.0, 1.0);
                    chan.fade = Crossfade::new(secs.max(0.0), sample_rate);
                }
                SoundCommand::EchoChannel(ch, secs, feedback, mix) => {
                    let c = ch.min(channels.len() - 1);
                    channels[c].echo.set(secs, feedback, mix, sample_rate);
                }
                SoundCommand::FilterChannel(ch, kind, cutoff, q, secs) => {
                    let c = ch.min(channels.len() - 1);
                    match kind {
                        Some(k) => channels[c].filter.set(k, cutoff, q, secs, sample_rate),
                        None => channels[c].filter.clear(),
                    }
                }
                SoundCommand::ReverbChannel(ch, room, damp, wet) => {
                    let c = ch.min(channels.len() - 1);
                    channels[c].reverb.set(room, damp, wet);
                }
                SoundCommand::CrushChannel(ch, bits, rate) => {
                    let c = ch.min(channels.len() - 1);
                    channels[c].crush.set(bits, rate, sample_rate);
                }
                SoundCommand::DriveChannel(ch, amount, shape) => {
                    let c = ch.min(channels.len() - 1);
                    channels[c].drive.set(amount, shape);
                }
                SoundCommand::SetLanes(counts) => {
                    // `attr{ lanes = {4,3,5} }`: set per-channel polyphony. Entry i
                    // (1-based in Lua) sets channel (i-1); unlisted channels keep
                    // their current lane count.
                    for (i, &n) in counts.iter().enumerate() {
                        if i < channels.len() {
                            channels[i].set_lanes(n);
                        }
                    }
                }
            }
        }

        let mut mix = 0.0f32;
        for chan in channels.iter_mut() {
            let mut ch_out = 0.0f32;
            for l in 0..chan.lanes() {
                // Start the next queued note when this lane is idle.
                if chan.current[l].is_none() {
                    if let Some(note) = chan.queues[l].pop_front() {
                        let inc = TWO_PI * note.frequency / sample_rate;
                        let mut voice = Voice::from_note(&note, inc);
                        // Sung note: set the vowel formants, or glide from a voiced
                        // consonant / diphthong start into them; stamp the channel's
                        // voice character (breath/vibrato) onto the voice.
                        if let Some(target) = &note.formants {
                            if let Some(from) = &note.glide_from {
                                voice
                                    .voice_bank
                                    .glide_to(from, target, note.glide_secs, sample_rate);
                            } else {
                                voice.voice_bank.set(target, sample_rate);
                            }
                            voice.vowel = *target; // for a voiced coda to glide from
                            voice.breath = chan.breath;
                            voice.vib_depth = chan.vib_depth;
                            voice.vib_rate = chan.vib_rate;
                        }
                        // Coda (played at note-off) + onset cluster: queue the
                        // onset bursts and start the first. Each is band-passed
                        // noise; the final onset burst crossfades into the vowel
                        // (up to 12ms) so it isn't an abrupt cut.
                        voice.coda_glide = note.coda_glide;
                        voice.coda = note.coda;
                        voice.onset = note.onset;
                        voice.onset_i = 0;
                        voice.onset_xfade_last = true;
                        if !voice.onset.is_empty() {
                            let is_last = voice.onset.len() == 1;
                            let k = voice.onset[0];
                            voice.onset_i = 1;
                            voice.load_burst(&k, is_last, sample_rate);
                        }
                        chan.current[l] = Some(voice);
                    }
                }

                // Sounding voice: run the attack→decay→sustain envelope, then hand
                // off to `fading` for release at note-off.
                if let Some(v) = chan.current[l].as_mut() {
                    let instr = instruments.get(&v.instrument).unwrap_or(&default_instr);
                    v.advance_env(&instr.env, sample_rate);
                    ch_out += voice_out(v, instr, &samples, sample_rate) * v.volume * v.env;
                    // One-shot samples play to their natural end, ignoring the note's
                    // duration — otherwise a pitched-DOWN sample (slower playback)
                    // gets cut off mid-buffer while still loud, an audible snap.
                    // voice_out sets remaining=0 when the buffer runs out, so the
                    // release still fires; oscillators use the duration timer.
                    if !matches!(instr.wave, WaveType::Sample(_)) {
                        v.remaining -= 1.0 / sample_rate;
                    }
                    if v.remaining <= 0.0 {
                        // Sustain (or a coda segment) ended. If there's still a
                        // coda to play (a voiced ending or unvoiced bursts), keep
                        // the voice sounding; otherwise release it. `advance_coda`
                        // is a no-op for notes without a coda.
                        if !v.advance_coda(sample_rate) {
                            // Note-off: move to release. Set the stage on the taken
                            // voice so we don't extend `v`'s borrow across the take.
                            if let Some(mut voice) = chan.current[l].take() {
                                voice.stage = EnvStage::Release;
                                chan.fading[l] = Some(voice);
                            }
                        }
                    }
                }

                // Fading voice: release ramp to silence at the instrument's rate.
                if let Some(v) = chan.fading[l].as_mut() {
                    let instr = instruments.get(&v.instrument).unwrap_or(&default_instr);
                    let rel = instr.env.release;
                    v.env -= if rel > 0.0 { 1.0 / (rel * sample_rate) } else { 1.0 };
                    if v.env <= 0.0 {
                        chan.fading[l] = None;
                    } else {
                        ch_out += voice_out(v, instr, &samples, sample_rate) * v.volume * v.env;
                    }
                }
            }

            // Channel effects, in chain order (like a pedal chain): drive and
            // bitcrush generate harmonics first, the filter then tames them, then
            // echo (discrete repeats) and reverb (diffuse tail) — both tails keep
            // ringing after the voices stop — then advance an in-flight fade and
            // apply the gain to the whole channel (dry + wet alike).
            ch_out = chan.drive.process(ch_out);
            ch_out = chan.crush.process(ch_out);
            ch_out = chan.filter.process(ch_out);
            ch_out = chan.echo.process(ch_out);
            ch_out = chan.reverb.process(ch_out);
            if !chan.fade.done() {
                chan.gain = chan.fade.mix(chan.fade_from, chan.fade_to);
            }
            mix += ch_out * chan.gain;
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

    // Advance the consonant queue: when the active burst is spent and another is
    // queued (an onset cluster like s+t, or the coda bursts), start the next.
    // The final onset burst crossfades into the vowel; coda bursts don't.
    if v.consonant_samples == 0 && v.consonant_xfade.done() && v.onset_i < v.onset.len() {
        let k = v.onset[v.onset_i];
        v.onset_i += 1;
        let xfade = v.onset_i >= v.onset.len() && v.onset_xfade_last;
        v.load_burst(&k, xfade, sr);
    }

    if v.consonant_samples > 0 {
        // Pure consonant: a band-passed noise burst.
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

#[derive(Clone, Debug, Serialize, Deserialize)]
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
    /// Onset consonant cluster: unvoiced noise bursts played in order before the
    /// vowel (e.g. s+t for "st").
    pub onset: Vec<Consonant>,
    /// Optional starting formants to glide *from* into `formants` (a voiced
    /// onset consonant, or a diphthong's first vowel), over `glide_secs`.
    pub glide_from: Option<[Formant; 3]>,
    pub glide_secs: f32,
    /// Optional voiced coda to glide *to* at note-off (a nasal/liquid ending
    /// like the "n" in "sun").
    pub coda_glide: Option<[Formant; 3]>,
    /// Coda consonant cluster: unvoiced bursts played after the vowel (the "t"
    /// in "cat", "t"+"s" in "cats").
    pub coda: Vec<Consonant>,
}
impl Note {
    pub fn new(instrument: usize, frequency: f32, duration: f32, volume: f32) -> Self {
        Self {
            instrument,
            frequency,
            duration,
            volume,
            formants: None,
            onset: Vec::new(),
            glide_from: None,
            glide_secs: 0.0,
            coda_glide: None,
            coda: Vec::new(),
        }
    }
    /// A sung note at `frequency` from a parsed syllable (onset cluster + vowel +
    /// optional glides + coda cluster).
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
            onset: syllable.onset.clone(),
            glide_from: syllable.glide_from,
            glide_secs: syllable.glide_secs,
            coda_glide: syllable.coda_glide,
            coda: syllable.coda.clone(),
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

/// How far a sung voice has progressed through its coda (the consonants after
/// the vowel) once its sustain ends. `None` until note-off; then a voiced glide
/// (nasal/liquid ending), then any unvoiced bursts, then `Done` → release.
#[derive(Clone, Copy, PartialEq)]
enum CodaStage {
    None,
    Glide,
    Bursts,
    Done,
}

/// One playback channel — a "track". It owns a pool of `lanes` monophonic voice
/// slots (its polyphony: a chord uses several lanes of the *same* channel, so a
/// chord's character/effects stay consistent), plus channel-level state that
/// applies to everything sounding on it: output `gain` with an in-flight `fade`,
/// and the singing-voice character (`breath`/`vibrato`) stamped onto sung notes
/// started here. Effects live on the channel; timbre/polyphony live in its lanes.
struct Channel {
    /// Per-lane pending-note queue (a phrase/song sequences on one lane).
    queues: Vec<VecDeque<Note>>,
    /// Per-lane sounding voice.
    current: Vec<Option<Voice>>,
    /// Per-lane releasing voice (ramping down so consecutive notes don't click).
    fading: Vec<Option<Voice>>,
    /// Round-robin lane to steal when all lanes are busy.
    steal: usize,
    /// Output gain (multiplies the whole channel's mix).
    gain: f32,
    /// In-flight gain ramp (a channel-level `fade`).
    fade: Crossfade,
    fade_from: f32,
    fade_to: f32,
    /// Channel-level waveshaping distortion (`grit`); a no-op until configured.
    drive: Drive,
    /// Channel-level bitcrusher (`crsh`); a no-op until configured.
    crush: Crush,
    /// Channel-level resonant filter (`filt`); a no-op until configured.
    filter: Filter,
    /// Channel-level feedback delay (`echo`); a no-op until configured.
    echo: Echo,
    /// Channel-level reverb (`verb`); a no-op until configured.
    reverb: Reverb,
    /// Singing-voice character applied to sung notes started on this channel.
    breath: f32,
    vib_depth: f32,
    vib_rate: f32,
}
impl Channel {
    /// Build a channel with `lanes` polyphony slots. Takes the device
    /// `sample_rate` so the buffer-owning effects (echo, reverb) can allocate
    /// their delay lines **here**, off the audio thread — see `Echo::new`.
    fn new(lanes: usize, sample_rate: f32) -> Self {
        let lanes = lanes.max(1);
        Self {
            queues: (0..lanes).map(|_| VecDeque::new()).collect(),
            current: (0..lanes).map(|_| None).collect(),
            fading: (0..lanes).map(|_| None).collect(),
            steal: 0,
            gain: 1.0,
            fade: Crossfade::default(),
            fade_from: 1.0,
            fade_to: 1.0,
            drive: Drive::default(),
            crush: Crush::default(),
            filter: Filter::default(),
            echo: Echo::new(sample_rate),
            reverb: Reverb::new(sample_rate),
            breath: 0.0,
            vib_depth: 0.0,
            vib_rate: 5.5,
        }
    }
    fn lanes(&self) -> usize {
        self.current.len()
    }
    /// Resize the lane pool (a config event via `attr{lanes}`, never the hot path).
    ///
    /// NOTE: these `resize_with`s allocate, and this runs on the audio thread —
    /// fine while `attr{lanes}` is a boot/config-time call, but if lanes ever
    /// become something games retune during play, preallocate a max lane pool
    /// (like `Echo`/`Reverb` do) and just move an active count instead.
    fn set_lanes(&mut self, lanes: usize) {
        let lanes = lanes.max(1);
        self.queues.resize_with(lanes, VecDeque::new);
        self.current.resize_with(lanes, || None);
        self.fading.resize_with(lanes, || None);
        if self.steal >= lanes {
            self.steal = 0;
        }
    }
    /// Pick a lane for a new note: the first fully-idle lane, else round-robin
    /// steal (the channel's polyphony cap = its lane count).
    fn pick_lane(&mut self) -> usize {
        let lanes = self.lanes();
        (0..lanes)
            .find(|&l| {
                self.current[l].is_none() && self.fading[l].is_none() && self.queues[l].is_empty()
            })
            .unwrap_or_else(|| {
                let l = self.steal;
                self.steal = (self.steal + 1) % lanes;
                l
            })
    }
    /// Release every lane (clear queues, ramp sounding voices down): `mute`/`Stop`.
    fn release_all(&mut self) {
        for l in 0..self.lanes() {
            self.queues[l].clear();
            if let Some(v) = self.current[l].take() {
                self.fading[l] = Some(v);
            }
        }
    }
    /// App reload: silence everything and restore channel defaults (incl. the
    /// default lane count, so a new game that never calls `attr` starts clean).
    fn reset(&mut self) {
        self.set_lanes(DEFAULT_LANES);
        for q in self.queues.iter_mut() {
            q.clear();
        }
        for c in self.current.iter_mut() {
            *c = None;
        }
        for f in self.fading.iter_mut() {
            *f = None;
        }
        self.steal = 0;
        self.gain = 1.0;
        self.fade = Crossfade::default();
        self.fade_from = 1.0;
        self.fade_to = 1.0;
        self.drive.clear();
        self.crush.clear();
        self.filter.clear();
        self.echo.clear();
        self.reverb.clear();
        self.breath = 0.0;
        self.vib_depth = 0.0;
        self.vib_rate = 5.5;
    }
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
    /// The active consonant burst: remaining samples of band-passed noise.
    consonant_samples: u32,
    consonant_filter: Biquad,
    /// Blends the *final* onset burst's noise into the vowel (skipped mid-cluster
    /// and for coda bursts, which have no vowel to land on).
    consonant_xfade: Crossfade,
    /// Consonant bursts still to play (onset cluster before the vowel, then reused
    /// for the coda cluster after it), advanced through by `onset_i`.
    onset: Vec<Consonant>,
    onset_i: usize,
    /// Whether the last burst in `onset` crossfades into the vowel (onset) or just
    /// ends (coda).
    onset_xfade_last: bool,
    /// The landed vowel formants, kept so a voiced coda can glide *from* them.
    vowel: [Formant; 3],
    /// Voiced coda ending to glide to at note-off (nasal/liquid), and the unvoiced
    /// coda bursts to play after it; `coda_stage` tracks progress.
    coda_glide: Option<[Formant; 3]>,
    coda: Vec<Consonant>,
    coda_stage: CodaStage,
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
            onset: Vec::new(),
            onset_i: 0,
            onset_xfade_last: true,
            vowel: [Formant::default(); 3],
            coda_glide: None,
            coda: Vec::new(),
            coda_stage: CodaStage::None,
            breath: 0.0,
            vib_depth: 0.0,
            vib_rate: 0.0,
            vib_phase: 0.0,
        }
    }
    /// Load the next queued consonant burst into the active-burst slot. The final
    /// onset burst crossfades into the vowel; mid-cluster and coda bursts don't.
    fn load_burst(&mut self, k: &Consonant, xfade_into_vowel: bool, sr: f32) {
        let total = (k.secs * sr) as u32;
        if xfade_into_vowel {
            let xfade_secs = (k.secs * 0.5).min(0.012);
            let xfade_samps = (xfade_secs * sr) as u32;
            self.consonant_samples = total.saturating_sub(xfade_samps);
            self.consonant_xfade = Crossfade::new(xfade_secs, sr);
        } else {
            self.consonant_samples = total.max(1);
            self.consonant_xfade = Crossfade::default(); // done → no blend
        }
        self.consonant_filter = Biquad::bandpass(k.freq, k.q, k.gain, sr);
    }
    /// Advance the coda one segment when the sustain (or a prior coda segment)
    /// runs out. Returns `true` if it set up more sound to play (a voiced glide
    /// or a burst run), `false` when the coda is finished and the voice should
    /// release. A no-op (returns `false`) for notes without a coda.
    fn advance_coda(&mut self, sr: f32) -> bool {
        loop {
            match self.coda_stage {
                CodaStage::None => self.coda_stage = CodaStage::Glide,
                CodaStage::Glide => {
                    self.coda_stage = CodaStage::Bursts;
                    if let Some(target) = self.coda_glide.take() {
                        // Glide the vowel into the nasal/liquid ending, staying tonal.
                        self.voice_bank.glide_to(&self.vowel, &target, 0.08, sr);
                        self.remaining = 0.08;
                        return true;
                    }
                }
                CodaStage::Bursts => {
                    self.coda_stage = CodaStage::Done;
                    if !self.coda.is_empty() {
                        // Reuse the onset burst machinery to play the coda bursts
                        // (no vowel to blend into → no final xfade).
                        self.onset = std::mem::take(&mut self.coda);
                        self.onset_i = 0;
                        self.onset_xfade_last = false;
                        self.consonant_samples = 0;
                        self.consonant_xfade = Crossfade::default();
                        let total: f32 = self.onset.iter().map(|k| k.secs).sum();
                        self.remaining = total.max(0.001);
                        return true;
                    }
                }
                CodaStage::Done => return false,
            }
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
    /// Play one note. `Some(ch)` targets a channel; `None` = channel 0 (the
    /// default track). The note takes a free lane of that channel, so overlapping
    /// notes on one channel form a chord (up to its lane count).
    PlayNote(Note, Option<usize>),
    /// Queue a sequence on a single lane of a channel (`None` = channel 0).
    Chain(Vec<Note>, Option<usize>),
    /// Release a channel — all its lanes (`None` = every channel).
    Stop(Option<usize>),
    /// Release the sounding voice at `frequency` on a channel (`None` = search
    /// every channel) — a real per-note *note-off*, as opposed to `Stop`, which
    /// cuts a whole channel. Backs MIDI note-off, where a held key must sustain
    /// until released; only the first match is released, so repeated notes at the
    /// same pitch unwind one per note-off.
    ReleaseNote(Option<usize>, f32),
    /// Ramp a channel's output gain: (channel, seconds, target gain 0..1).
    FadeChannel(usize, f32, f32),
    /// Set a channel's echo (feedback delay): (channel, delay secs, feedback
    /// 0..1, wet mix). `secs <= 0` disables it.
    EchoChannel(usize, f32, f32, f32),
    /// Set a channel's resonant filter: (channel, kind or `None` = off, cutoff
    /// Hz, resonance q, sweep secs — 0 = immediate). Backs `filt`.
    FilterChannel(usize, Option<FilterKind>, f32, f32, f32),
    /// Set a channel's reverb: (channel, room/decay 0..1, damping 0..1, wet mix).
    /// `room <= 0` disables it. Backs `verb`.
    ReverbChannel(usize, f32, f32, f32),
    /// Set a channel's bitcrusher: (channel, bit depth, target sample-rate Hz).
    /// `bits <= 0` disables it. Backs `crsh`.
    CrushChannel(usize, f32, f32),
    /// Set a channel's drive/distortion: (channel, amount 0..1, curve).
    /// `amount <= 0` disables it. Backs `grit`.
    DriveChannel(usize, f32, DriveShape),
    /// Set a channel's singing-voice character for its later sung notes:
    /// (channel, breath, vibrato depth as a pitch fraction, vibrato rate Hz).
    VoiceConfig(usize, f32, f32, f32),
    /// Set per-channel polyphony lane counts: entry i sets channel i (`attr{lanes}`).
    SetLanes(Vec<usize>),
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


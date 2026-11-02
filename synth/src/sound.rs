use std::{
    collections::VecDeque,
    sync::{
        mpsc::{channel, Receiver, Sender},
        Arc,
    },
};

#[cfg(all(feature = "host-io", not(target_arch = "wasm32")))]
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use serde::{Deserialize, Serialize};
use rustc_hash::FxHashMap;

use crate::fx::{Biquad, Crossfade, Crush, Drive, DriveShape, Echo, Filter, FilterKind, Reverb};
use crate::vocaloid::{Cluster, Consonant, Formant, FormantBank};

#[cfg(all(feature = "host-io", not(target_arch = "wasm32")))]
#[derive(Debug)]
struct Opt {
    #[cfg(all(
        any(target_os = "linux", target_os = "dragonfly", target_os = "freebsd"),
        feature = "jack"
    ))]
    jack: bool,

    device: String,
}

#[cfg(all(feature = "host-io", not(target_arch = "wasm32")))]
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

#[cfg(all(feature = "host-io", not(target_arch = "wasm32")))]
pub fn init() -> (anyhow::Result<cpal::Stream>, Sender<SoundCommand>) {
    let (singer, audience) = channel::<SoundCommand>();
    (init_sound(audience), singer)
}

#[cfg(all(feature = "host-io", not(target_arch = "wasm32")))]
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

/// Name the captured microphone audio is filed under in the loaded-sound bank, so
/// `smpl(id, 'mic')` can rebind the last recording to more slots.
pub const MIC_BANK_NAME: &str = "mic";

/// Longest single microphone capture, in seconds — a snippet sampler, not a tape
/// deck. The record buffer is preallocated to this at capture start.
pub const MAX_MIC_SECS: f32 = 10.0;

/// Open the default audio **input** device and record one mono snippet.
///
/// Captures `secs` of audio, then hands it to the synth as a playable sample: the
/// PCM goes into the loaded-sound bank under `MIC_BANK_NAME` (which carries the
/// input device's own sample rate, so playback pitch is right even when the input
/// and output devices run at different rates) and is bound to instrument slot
/// `id`. After that, `note(440, len, ch, id)` plays the recording at its natural
/// speed — 440 being the base pitch the binding uses.
///
/// The returned stream must be kept alive for the duration; `done` flips to true
/// once the snippet has been sent, so the caller can drop the stream and release
/// the microphone (rather than holding it open and leaving the OS "mic in use"
/// indicator lit).
#[cfg(all(feature = "host-io", not(target_arch = "wasm32")))]
pub fn record_mic(
    id: usize,
    secs: f32,
    singer: Sender<SoundCommand>,
    done: Arc<std::sync::atomic::AtomicBool>,
) -> anyhow::Result<cpal::Stream> {
    let host = cpal::default_host();
    let device = host
        .default_input_device()
        .ok_or_else(|| anyhow::anyhow!("no audio input device found"))?;
    let config = device.default_input_config()?;
    let sample_rate = config.sample_rate().0 as f32;
    let channels = config.channels() as usize;
    let secs = secs.clamp(0.05, MAX_MIC_SECS);
    let target = (secs * sample_rate) as usize;
    log::info!(
        "mic: {} Hz, {} ch, capturing {:.2}s into slot {}",
        sample_rate,
        channels,
        secs,
        id
    );

    let stream_config: cpal::StreamConfig = config.clone().into();
    let err_fn = |err| log::warn!("mic stream error: {}", err);
    match config.sample_format() {
        cpal::SampleFormat::F32 => capture::<f32>(
            &device, &stream_config, channels, target, id, sample_rate, singer, done, err_fn,
        ),
        cpal::SampleFormat::I16 => capture::<i16>(
            &device, &stream_config, channels, target, id, sample_rate, singer, done, err_fn,
        ),
        cpal::SampleFormat::U16 => capture::<u16>(
            &device, &stream_config, channels, target, id, sample_rate, singer, done, err_fn,
        ),
        cpal::SampleFormat::I32 => capture::<i32>(
            &device, &stream_config, channels, target, id, sample_rate, singer, done, err_fn,
        ),
        cpal::SampleFormat::F64 => capture::<f64>(
            &device, &stream_config, channels, target, id, sample_rate, singer, done, err_fn,
        ),
        other => Err(anyhow::anyhow!("unsupported mic sample format {:?}", other)),
    }
}

/// Build the input stream for one capture. The callback owns its record buffer
/// (preallocated here, off the audio thread), downmixes to mono, and when it has
/// `target` samples ships them to the synth and sets `done` — so nothing is
/// allocated or locked in the input callback itself.
#[cfg(all(feature = "host-io", not(target_arch = "wasm32")))]
#[allow(clippy::too_many_arguments)]
fn capture<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    channels: usize,
    target: usize,
    id: usize,
    sample_rate: f32,
    singer: Sender<SoundCommand>,
    done: Arc<std::sync::atomic::AtomicBool>,
    err_fn: impl FnMut(cpal::StreamError) + Send + 'static,
) -> anyhow::Result<cpal::Stream>
where
    T: cpal::SizedSample + Send + 'static,
    f32: cpal::FromSample<T>,
{
    use std::sync::atomic::Ordering;
    let mut buf: Vec<f32> = Vec::with_capacity(target);
    let mut sent = false;
    let stream = device.build_input_stream(
        config,
        move |data: &[T], _: &cpal::InputCallbackInfo| {
            if sent {
                return;
            }
            // Downmix interleaved frames to mono; we only ever sample in mono.
            for frame in data.chunks(channels.max(1)) {
                if buf.len() >= target {
                    break;
                }
                let mut sum = 0.0f32;
                for s in frame {
                    sum += <f32 as cpal::FromSample<T>>::from_sample_(*s);
                }
                buf.push(sum / channels.max(1) as f32);
            }
            if buf.len() >= target {
                // Snippet complete: file it in the name bank (which normalizes it
                // and keeps the input rate for correct playback pitch) and bind it
                // to the requested slot. One move, no copy of the PCM.
                let pcm = std::mem::take(&mut buf);
                let _ = singer.send(SoundCommand::LoadSample(
                    MIC_BANK_NAME.to_string(),
                    pcm,
                    sample_rate,
                ));
                let _ = singer.send(SoundCommand::BindSample(
                    id,
                    MIC_BANK_NAME.to_string(),
                    Some(440.0),
                    Envelope::default(),
                    (0.0, 1.0),
                ));
                sent = true;
                done.store(true, Ordering::Release);
            }
        },
        err_fn,
        None,
    )?;
    stream.play()?;
    Ok(stream)
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
/// The mixer as a trait object, for hosts that must store it behind a `Box` (the
/// browser outputs: the main-thread `WebAudioOut` fallback and the AudioWorklet).
pub fn make_mixer_boxed(
    sample_rate: f32,
    audience: Receiver<SoundCommand>,
) -> Box<dyn FnMut() -> f32> {
    Box::new(make_mixer(sample_rate, audience))
}

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

    // The transport: a monotonic frame counter and the tempo it is read against.
    // `Note.duration` was already counted in samples, so a chain never drifted from
    // itself — what was missing was a *start* time, which left every trigger
    // quantised to whichever 60Hz Lua frame sent it.
    let mut frames: u64 = 0;
    let mut bpm: f32 = 120.0;
    // Scheduled notes waiting for their frame. A fixed array because the audio
    // thread must not allocate or free; `Note` is Copy, so this costs nothing to
    // fill. Overflow drops the newest and says so rather than growing.
    const MAX_SCHEDULED: usize = 64;
    let mut pending: [Option<(u64, Note, usize)>; MAX_SCHEDULED] = [None; MAX_SCHEDULED];
    // The earliest frame anything is due, so the common case is one comparison per
    // sample instead of a scan of all 64 slots.
    let mut next_due: u64 = u64::MAX;
    // Running sequences, and the earliest frame any of them has a step due.
    let mut seqs: Vec<Playing> = Vec::with_capacity(MAX_SEQS);
    let mut seq_due: u64 = u64::MAX;

    move || -> f32 {
        // Drain every pending command each sample so triggers are effectively
        // sample-accurate (the old code polled once per ~2000 samples, which
        // quantised note timing and could drop/merge fast notes).
        while let Ok(cmd) = audience.try_recv() {
            match cmd {
                SoundCommand::SetBpm(b) => {
                    if b > 0.0 {
                        bpm = b;
                        // Sequences keep their place in beats; their frames move.
                        seq_due = 0;
                    }
                }
                SoundCommand::Sequence(seq) => {
                    if seq.steps.is_empty() || seq.length <= 0.0 {
                        continue;
                    }
                    let fpb = 60.0 / bpm as f64 * sample_rate as f64;
                    let now_beat = frames as f64 / fpb;
                    let mut start = now_beat;
                    match seq.at {
                        Some(a) => {
                            start = now_beat.max(a as f64);
                            // An explicit start replaces the old pattern outright.
                            seqs.retain(|p| p.seq.id != seq.id || p.ghost);
                        }
                        None => {
                            // Same id already running: hand over at the end of its
                            // pass, so a pattern change never lands mid-bar.
                            seqs.retain(|p| p.seq.id != seq.id || !p.waiting);
                            if let Some(k) =
                                seqs.iter().position(|p| p.seq.id == seq.id && !p.ghost)
                            {
                                let old = &mut seqs[k];
                                let len = old.seq.length as f64;
                                if old.next == 0 {
                                    // Nothing of this pass has played yet: swap now.
                                    start = old.start + old.pass as f64 * len;
                                    seqs.swap_remove(k);
                                } else {
                                    start = old.start + (old.pass + 1) as f64 * len;
                                    old.seq.loops = old.pass + 1;
                                    old.ghost = true;
                                }
                            }
                        }
                    }
                    if let Some(g) = seq.grid.filter(|g| *g > 0.0) {
                        let g = g as f64;
                        // A hair of slack so a start already on the grid stays put.
                        start = ((start - 1e-6) / g).ceil() * g;
                    }
                    let waiting = seq
                        .after
                        .map(|a| seqs.iter().any(|p| p.seq.id == a && !p.ghost))
                        .unwrap_or(false);
                    if seqs.len() >= MAX_SEQS {
                        eprintln!("sound: too many sequences, {} dropped", seq.id);
                        continue;
                    }
                    seqs.push(Playing {
                        seq,
                        start,
                        pass: 0,
                        next: 0,
                        waiting,
                        ghost: false,
                    });
                    seq_due = 0;
                }
                SoundCommand::StopSequence(id) => match id {
                    Some(id) => seqs.retain(|p| p.seq.id != id),
                    None => seqs.clear(),
                },
                SoundCommand::PlayAt(note, ch, at_beat, quant) => {
                    let c = ch.unwrap_or(0).min(channels.len() - 1);
                    let fpb = 60.0 / bpm * sample_rate;
                    let now_beat = frames as f32 / fpb;
                    let mut target = at_beat.max(now_beat);
                    if let Some(g) = quant {
                        if g > 0.0 {
                            target = (target / g).ceil() * g;
                        }
                    }
                    let at = (target * fpb) as u64;
                    match pending.iter_mut().find(|slot| slot.is_none()) {
                        Some(slot) => {
                            *slot = Some((at, note, c));
                            if at < next_due {
                                next_due = at;
                            }
                        }
                        None => eprintln!("sound: schedule queue full, note dropped"),
                    }
                }
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
                    let end = pcm.len();
                    samples.insert(
                        id,
                        Sample {
                            pcm: pcm.into(),
                            base_freq,
                            rate_ratio: 1.0,
                            start: 0,
                            end,
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
                SoundCommand::BindSample(id, name, base, env, (from, to)) => {
                    // `smpl(id, 'name')`: resolve the loaded file once and copy
                    // its (Arc) buffer into integer slot `id`. Missing name =>
                    // leave the slot as-is (the note falls back to a default).
                    if let Some((pcm, source_rate)) = loaded.get(&name) {
                        let n = pcm.len();
                        let start = ((from.clamp(0.0, 1.0) * n as f32) as usize).min(n);
                        let end = ((to.clamp(0.0, 1.0) * n as f32) as usize).clamp(start, n);
                        samples.insert(
                            id,
                            Sample {
                                pcm: Arc::clone(pcm),
                                base_freq: base.filter(|b| *b > 0.0).unwrap_or(440.0),
                                rate_ratio: source_rate / sample_rate,
                                start,
                                end,
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
                    // The transport is persistent audio-thread state like the rest:
                    // a new app must not inherit the old one's tempo, and notes it
                    // never scheduled must not fire into it.
                    bpm = 120.0;
                    pending = [None; MAX_SCHEDULED];
                    next_due = u64::MAX;
                    seqs.clear();
                    seq_due = u64::MAX;
                    frames = 0;
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

        // Fire anything the transport has reached. Guarded by `next_due` so a quiet
        // stretch costs one comparison per sample rather than a scan of every slot.
        if frames >= next_due {
            next_due = u64::MAX;
            for slot in pending.iter_mut() {
                match slot {
                    Some((at, note, c)) if *at <= frames => {
                        let (note, c) = (*note, *c);
                        *slot = None;
                        let l = channels[c].pick_lane();
                        channels[c].queues[l].push_back(note);
                    }
                    Some((at, _, _)) => {
                        if *at < next_due {
                            next_due = *at;
                        }
                    }
                    None => {}
                }
            }
        }
        // Walk the sequences the transport has reached. Same guard as above: one
        // comparison per sample until a step is due.
        if frames >= seq_due {
            seq_due = u64::MAX;
            let fpb = 60.0 / bpm as f64 * sample_rate as f64;
            let secs_per_beat = 60.0 / bpm;
            let mut i = 0;
            while i < seqs.len() {
                if seqs[i].waiting {
                    i += 1;
                    continue;
                }
                let mut finished = false;
                loop {
                    let p = &mut seqs[i];
                    let due = (p.next_beat() * fpb) as u64;
                    if due > frames {
                        seq_due = seq_due.min(due);
                        break;
                    }
                    let step = p.seq.steps[p.next];
                    let c = p.seq.channel.min(channels.len() - 1);
                    if p.seq.choke {
                        channels[c].release_all();
                    }
                    // A rest is a step with no pitch: it holds its place and sounds
                    // nothing.
                    if step.note.frequency > 0.0 {
                        let mut note = step.note;
                        note.duration = step.len * secs_per_beat;
                        channels[c].start_now(note);
                    }
                    p.next += 1;
                    if p.next >= p.seq.steps.len() {
                        p.next = 0;
                        p.pass += 1;
                        if p.seq.loops > 0 && p.pass >= p.seq.loops {
                            finished = true;
                            break;
                        }
                    }
                }
                if finished {
                    let done = seqs.swap_remove(i);
                    if !done.ghost {
                        // Wake whatever was chained to it, starting on its last beat.
                        let end = done.end_beat();
                        for w in seqs.iter_mut() {
                            if w.waiting && w.seq.after == Some(done.seq.id) {
                                w.waiting = false;
                                w.start = end;
                                if let Some(g) = w.seq.grid.filter(|g| *g > 0.0) {
                                    let g = g as f64;
                                    w.start = ((end - 1e-6) / g).ceil() * g;
                                }
                            }
                        }
                    }
                    // A woken sequence may be due this very frame: look again next
                    // sample rather than reasoning about the swapped-in element.
                    seq_due = frames;
                    continue;
                }
                i += 1;
            }
        }
        frames += 1;

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
                                voice.voice_bank.glide_to(
                                    from,
                                    target,
                                    note.glide_secs,
                                    note.frequency,
                                    sample_rate,
                                );
                            } else {
                                voice.voice_bank.set(target, note.frequency, sample_rate);
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
                        // Short syllables keep their vowel: the onset gets at most a
                        // fifth of the note and the coda a seventh (no-op at sung
                        // lengths). At a third each side, a 70ms talk blip like
                        // "shoot" was two-thirds hiss and read as whispered.
                        voice.onset.squeeze(note.duration * 0.2);
                        voice.coda.squeeze(note.duration * 0.15);
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
                // `mix` returns the blend *before* its last step, so land on the
                // target exactly: otherwise `fade(ch, 0, x)` never reaches x.
                if chan.fade.done() {
                    chan.gain = chan.fade_to;
                }
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
#[cfg(all(feature = "host-io", not(target_arch = "wasm32")))]
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
    // Dev aids for checking sound without ears: `CHOR_WAV=path` records the master
    // bus to a mono WAV, and `CHOR_MUTE=1` sends silence to the device while the
    // mixer (and the recording) runs as normal, so a test needn't play aloud.
    let tap = std::env::var("CHOR_WAV").ok().map(|p| wav_tap(p, config.sample_rate.0));
    let mute = std::env::var("CHOR_MUTE").map(|v| v == "1").unwrap_or(false);
    let mut block: Vec<f32> = Vec::new();
    let stream = device.build_output_stream(
        config,
        move |data: &mut [T], _: &cpal::OutputCallbackInfo| {
            if tap.is_none() && !mute {
                write_data(data, out_channels, &mut next_value);
                return;
            }
            // Debug path only: this allocates on the audio thread.
            block.clear();
            let mut capture = || {
                let v = next_value();
                block.push(v);
                if mute {
                    0.0
                } else {
                    v
                }
            };
            write_data(data, out_channels, &mut capture);
            if let Some(tx) = &tap {
                let _ = tx.try_send(block.clone());
            }
        },
        err_fn,
        None,
    )?;
    stream.play()?;
    Ok(stream)
}

/// Start a thread that appends mono f32 blocks to a 16-bit WAV at `path`,
/// rewriting the header's sizes after each block so the file is valid whenever
/// the app is killed. Backs `CHOR_WAV`.
#[cfg(all(feature = "host-io", not(target_arch = "wasm32")))]
fn wav_tap(path: String, rate: u32) -> std::sync::mpsc::SyncSender<Vec<f32>> {
    use std::io::{Seek, SeekFrom, Write};
    let (tx, rx) = std::sync::mpsc::sync_channel::<Vec<f32>>(256);
    std::thread::spawn(move || {
        let mut f = match std::fs::File::create(&path) {
            Ok(f) => f,
            Err(e) => {
                eprintln!("CHOR_WAV: can't create {}: {}", path, e);
                return;
            }
        };
        let header = |data_len: u32| {
            let mut h = Vec::with_capacity(44);
            h.extend_from_slice(b"RIFF");
            h.extend_from_slice(&(36 + data_len).to_le_bytes());
            h.extend_from_slice(b"WAVEfmt ");
            h.extend_from_slice(&16u32.to_le_bytes());
            h.extend_from_slice(&1u16.to_le_bytes()); // PCM
            h.extend_from_slice(&1u16.to_le_bytes()); // mono
            h.extend_from_slice(&rate.to_le_bytes());
            h.extend_from_slice(&(rate * 2).to_le_bytes());
            h.extend_from_slice(&2u16.to_le_bytes());
            h.extend_from_slice(&16u16.to_le_bytes());
            h.extend_from_slice(b"data");
            h.extend_from_slice(&data_len.to_le_bytes());
            h
        };
        let _ = f.write_all(&header(0));
        let mut len: u32 = 0;
        for block in rx {
            let bytes: Vec<u8> = block
                .iter()
                .flat_map(|v| ((v.clamp(-1.0, 1.0) * 32767.0) as i16).to_le_bytes())
                .collect();
            len += bytes.len() as u32;
            let _ = f.seek(SeekFrom::End(0));
            let _ = f.write_all(&bytes);
            let _ = f.seek(SeekFrom::Start(0));
            let _ = f.write_all(&header(len));
        }
    });
    eprintln!("CHOR_WAV: recording the master bus");
    tx
}

/// Web audio output: schedule short PCM chunks onto the AudioContext's own clock
/// ("two clocks" technique), so playback is glitch-free through main-thread jank
/// as long as we keep `lookahead` seconds queued. cpal's WebAudio backend ran on
/// the main thread and crackled; this bypasses it entirely.
#[cfg(all(feature = "host-io", feature = "web-fallback", target_arch = "wasm32"))]
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

#[cfg(all(feature = "host-io", feature = "web-fallback", target_arch = "wasm32"))]
impl WebAudioOut {
    pub fn new(audience: Receiver<SoundCommand>) -> Result<Self, wasm_bindgen::JsValue> {
        Self::with_context(web_sys::AudioContext::new()?, audience)
    }

    /// Build on an **existing** AudioContext. Important when this is used as the
    /// worklet path's fallback: browsers start a context suspended until a user
    /// gesture, and the engine's gesture handler only knows about the context it
    /// was given (`WebOut::context`). A fallback that made its own context would
    /// never be resumed, and the page would sit silent.
    pub fn with_context(
        ctx: web_sys::AudioContext,
        audience: Receiver<SoundCommand>,
    ) -> Result<Self, wasm_bindgen::JsValue> {
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
#[cfg(all(feature = "host-io", feature = "web-fallback", target_arch = "wasm32"))]
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
    /// The slice of `pcm` this slot plays, as frame indices `start..end`. A whole
    /// buffer is `0..len`; `smpl(id, 'amen', {from, to})` binds a piece of one, so
    /// a break chops into slots that all share the loaded file's `Arc`.
    start: usize,
    end: usize,
}

/// PolyBLEP correction for a ramp's reset at phase `t` (0..1) stepping `dt` per
/// sample: subtract from a naive sawtooth to band-limit its edge.
fn poly_blep(t: f32, dt: f32) -> f32 {
    if dt <= 0.0 {
        0.0
    } else if t < dt {
        let x = t / dt;
        x + x - x * x - 1.0
    } else if t > 1.0 - dt {
        let x = (t - 1.0) / dt;
        x * x + x + x + 1.0
    } else {
        0.0
    }
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
            let i = s.start + v.sample_pos as usize;
            let out = if i + 1 < s.end {
                let frac = v.sample_pos.fract();
                s.pcm[i] * (1.0 - frac) + s.pcm[i + 1] * frac
            } else if i < s.end {
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
        // Glottal source: a band-limited (PolyBLEP) sawtooth. The naive ramp
        // aliases badly at sung pitches, and the folded-back partials land
        // between the harmonics as a gritty hiss of their own.
        let t = v.phase / TWO_PI;
        let dt = v.phase_inc / TWO_PI;
        let saw = 2.0 * t - 1.0 - poly_blep(t, dt);
        // Breath: aspiration noise mixed into the source, then shaped by the same
        // formants — a breathy vowel rather than a hiss on top. It pulses with
        // each glottal cycle (loudest just after the reset, where the folds open),
        // as real aspiration does; steady noise is heard as a separate whisper
        // beside the voice instead of as part of it.
        let pulse = 0.35 + 0.65 * (0.5 + 0.5 * v.phase.cos());
        let src = saw + v.breath * pulse * white_noise(&mut v.rng);
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

#[cfg(all(feature = "host-io", not(target_arch = "wasm32")))]
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

/// `Copy` again, now that the consonant clusters are fixed-size: a note is plain
/// data the audio thread can take by value and forget, with nothing to free.
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
    /// Onset consonant cluster: unvoiced noise bursts played in order before the
    /// vowel (e.g. s+t for "st").
    pub onset: Cluster,
    /// Optional starting formants to glide *from* into `formants` (a voiced
    /// onset consonant, or a diphthong's first vowel), over `glide_secs`.
    pub glide_from: Option<[Formant; 3]>,
    pub glide_secs: f32,
    /// Optional voiced coda to glide *to* at note-off (a nasal/liquid ending
    /// like the "n" in "sun").
    pub coda_glide: Option<[Formant; 3]>,
    /// Coda consonant cluster: unvoiced bursts played after the vowel (the "t"
    /// in "cat", "t"+"s" in "cats").
    pub coda: Cluster,
}
impl Note {
    pub fn new(instrument: usize, frequency: f32, duration: f32, volume: f32) -> Self {
        Self {
            instrument,
            frequency,
            duration,
            volume,
            formants: None,
            onset: Cluster::default(),
            glide_from: None,
            glide_secs: 0.0,
            coda_glide: None,
            coda: Cluster::default(),
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
            onset: syllable.onset,
            glide_from: syllable.glide_from,
            glide_secs: syllable.glide_secs,
            coda_glide: syllable.coda_glide,
            coda: syllable.coda,
        }
    }
}

/// One step of a [`Sequence`]: a note placed on the pattern's beat grid.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Step {
    /// Where the step starts, in beats from the top of the pattern.
    pub at: f32,
    /// How long it sounds, in beats. Stays in beats until the step fires and is
    /// converted there, because the tempo belongs to the audio thread.
    pub len: f32,
    pub note: Note,
}

/// A sequence (`seq`): steps on a beat grid, played once, a counted number of
/// times, or forever, on one channel. The audio thread walks it against the
/// transport, so every pass lands on the sample clock — a looped break never
/// drifts from the bass under it, which a Lua loop calling `note` each frame
/// would.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Sequence {
    /// The caller's handle. Sending the same id again swaps the pattern at the end
    /// of the current pass; `after` names another sequence by it.
    pub id: usize,
    /// Sorted by `at`. Steps sharing an `at` sound together (a chord).
    pub steps: Vec<Step>,
    /// Beats per pass.
    pub length: f32,
    /// How many passes to play. `0` = forever.
    pub loops: u32,
    pub channel: usize,
    /// Absolute transport beat to start on; `None` = now (or the end of the pass,
    /// when replacing a running sequence of the same id).
    pub at: Option<f32>,
    /// Snap the start up to the next multiple of this many beats.
    pub grid: Option<f32>,
    /// Start when the sequence with this id finishes, rather than now. A chained
    /// intro → verse → loop is a few `seq` calls with `after`.
    pub after: Option<usize>,
    /// Cut whatever is still sounding on the channel when a step fires: chopped
    /// breaks want each slice to stop the last, the way a sampler's choke group
    /// does.
    pub choke: bool,
}

/// A sequence on the audio thread: where it started and how far it has got.
struct Playing {
    seq: Sequence,
    /// Transport beat of the first pass's beat 0.
    start: f64,
    pass: u32,
    /// Index of the next step to fire.
    next: usize,
    /// Chained with `after` and its predecessor is still playing.
    waiting: bool,
    /// Superseded by a newer sequence of the same id: plays out its current pass,
    /// then ends without starting anything chained to the id.
    ghost: bool,
}

impl Playing {
    fn next_beat(&self) -> f64 {
        self.start
            + self.pass as f64 * self.seq.length as f64
            + self.seq.steps[self.next].at as f64
    }
    /// The beat this sequence's last pass ends on (only meaningful when counted).
    fn end_beat(&self) -> f64 {
        self.start + self.seq.loops as f64 * self.seq.length as f64
    }
}

/// Most sequences playing at once; the store is preallocated so starting one
/// doesn't grow it on the audio thread.
const MAX_SEQS: usize = 32;

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
    /// Start a note on this frame. `pick_lane` steals a busy lane when they're all
    /// taken, and a stolen lane's queue would hold the note until its voice ends —
    /// fine for `note`, but a sequencer step that starts late is a wrong note. So
    /// the voice in the way is sent to its release instead.
    fn start_now(&mut self, note: Note) {
        let l = self.pick_lane();
        if let Some(v) = self.current[l].take() {
            self.fading[l] = Some(v);
        }
        self.queues[l].clear();
        self.queues[l].push_back(note);
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
    onset: Cluster,
    onset_i: usize,
    /// Whether the last burst in `onset` crossfades into the vowel (onset) or just
    /// ends (coda).
    onset_xfade_last: bool,
    /// The landed vowel formants, kept so a voiced coda can glide *from* them.
    vowel: [Formant; 3],
    /// Voiced coda ending to glide to at note-off (nasal/liquid), and the unvoiced
    /// coda bursts to play after it; `coda_stage` tracks progress.
    coda_glide: Option<[Formant; 3]>,
    coda: Cluster,
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
            onset: Cluster::default(),
            onset_i: 0,
            onset_xfade_last: true,
            vowel: [Formant::default(); 3],
            coda_glide: None,
            coda: Cluster::default(),
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
                        self.voice_bank
                            .glide_to(&self.vowel, &target, 0.08, self.freq, sr);
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
    /// Transport tempo, shared by every channel. One clock with per-channel
    /// subdivisions is what gives polyrhythm; independent tempos per channel would
    /// only give drift (see PLAN.md, sound clock).
    SetBpm(f32),
    /// Play a note at a point on the transport rather than on arrival.
    ///
    /// `at_beat` is an absolute transport beat, clamped forward to now — a command
    /// cannot land in the past. `quant`, if set, snaps that up to the next multiple
    /// of itself (1.0 = next beat, 0.25 = next sixteenth), which is what makes a
    /// trigger fired from a jittery 60Hz Lua frame land exactly on the grid.
    ///
    /// Beats are converted to frames *here*, on the audio thread, so tempo has one
    /// owner and a bpm change cannot race a scheduled note.
    PlayAt(Note, Option<usize>, f32, Option<f32>),
    /// Start (or swap in) a sequence. Backs `seq(id, steps, cfg)`.
    Sequence(Sequence),
    /// Stop a sequence by id (`None` = all). Notes already sounding ring out.
    StopSequence(Option<usize>),
    MakeInstrument(Instrument),
    /// Store a PCM sample (id, mono samples -1..1, base pitch, envelope) and
    /// register a same-id instrument that plays it.
    MakeSample(usize, Vec<f32>, f32, Envelope),
    /// Stash a disk-loaded sound in the name bank (name, mono PCM, source
    /// sample-rate). Sent by the asset loader at boot; bound later by BindSample.
    LoadSample(String, Vec<f32>, f32),
    /// Bind a name-banked sound into integer instrument slot `id`, optionally
    /// overriding its base pitch, with an envelope and the slice of the file to
    /// play (`from`, `to` as 0..1 fractions; `(0, 1)` = all of it). Backs
    /// `smpl(id, 'name', cfg?)`.
    BindSample(usize, String, Option<f32>, Envelope, (f32, f32)),
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


#[cfg(test)]
mod tests {
    use super::*;

    /// Pull `n` samples from a fresh mixer and return the peak amplitude.
    fn peak_of(cmds: Vec<SoundCommand>, n: usize) -> f32 {
        let (tx, rx) = channel::<SoundCommand>();
        for c in cmds {
            tx.send(c).unwrap();
        }
        let mut mixer = make_mixer_boxed(48000.0, rx);
        let mut peak = 0.0f32;
        for _ in 0..n {
            peak = peak.max(mixer().abs());
        }
        peak
    }

    /// Step the mixer and report the first sample index at which it makes sound.
    /// The point of the transport is *when* a note starts, so a peak test cannot
    /// check it — this can.
    fn first_sound_at(cmds: Vec<SoundCommand>, n: usize) -> Option<usize> {
        let (tx, rx) = channel::<SoundCommand>();
        for c in cmds {
            tx.send(c).unwrap();
        }
        let mut mixer = make_mixer_boxed(48000.0, rx);
        (0..n).find(|_| mixer().abs() > 0.001)
    }

    /// A scheduled note starts on the frame its beat lands on, not when the command
    /// arrived. At 120bpm a beat is 24000 frames, so beat 1 is frame 24000.
    #[test]
    fn a_scheduled_note_starts_on_its_beat() {
        let at = first_sound_at(
            vec![
                SoundCommand::SetBpm(120.0),
                SoundCommand::PlayAt(Note::new(0, 440.0, 0.5, 1.0), None, 1.0, None),
            ],
            48000,
        )
        .expect("scheduled note never sounded");
        // Allowing a few frames: the voice's attack ramp has to clear the threshold.
        assert!(
            (23_990..=24_100).contains(&at),
            "expected the note at ~frame 24000 (beat 1 at 120bpm), got {}",
            at
        );
    }

    /// Tempo is read on the audio thread, so the same beat means a different frame
    /// at a different bpm — 240bpm halves it.
    #[test]
    fn tempo_scales_the_schedule() {
        let at = first_sound_at(
            vec![
                SoundCommand::SetBpm(240.0),
                SoundCommand::PlayAt(Note::new(0, 440.0, 0.5, 1.0), None, 1.0, None),
            ],
            48000,
        )
        .expect("scheduled note never sounded");
        assert!(
            (11_990..=12_100).contains(&at),
            "expected ~frame 12000 (beat 1 at 240bpm), got {}",
            at
        );
    }

    /// Quantising is the point of the whole thing: a trigger fired at an arbitrary
    /// moment — as a 60Hz game loop does — must still land on the grid. Beat 0 with a
    /// one-beat grid snaps to the next whole beat.
    #[test]
    fn quantise_snaps_a_late_trigger_onto_the_grid() {
        let at = first_sound_at(
            vec![
                SoundCommand::SetBpm(120.0),
                SoundCommand::PlayAt(Note::new(0, 440.0, 0.5, 1.0), None, 0.0, Some(1.0)),
            ],
            48000,
        )
        .expect("scheduled note never sounded");
        // Frame 0 is exactly on the grid, so "next beat" is beat 0 itself.
        assert!(at < 100, "expected an immediate start on the grid, got {}", at);
    }

    /// A note scheduled for a beat already gone must not be dropped or fire in the
    /// past — it plays now.
    #[test]
    fn a_past_beat_plays_immediately() {
        let at = first_sound_at(
            vec![
                SoundCommand::SetBpm(120.0),
                SoundCommand::PlayAt(Note::new(0, 440.0, 0.5, 1.0), None, -5.0, None),
            ],
            4800,
        )
        .expect("a past-dated note was dropped");
        assert!(at < 100, "expected an immediate start, got {}", at);
    }

    /// Frames (at 48kHz) where sound starts after a stretch of silence: one entry
    /// per note a sequence fired, as long as the notes are shorter than the gaps.
    fn onsets(cmds: Vec<SoundCommand>, n: usize) -> Vec<usize> {
        let (tx, rx) = channel::<SoundCommand>();
        for c in cmds {
            tx.send(c).unwrap();
        }
        let mut mixer = make_mixer_boxed(48000.0, rx);
        let mut out = Vec::new();
        let mut quiet = 1000;
        for i in 0..n {
            if mixer().abs() > 0.001 {
                if quiet >= 1000 {
                    out.push(i);
                }
                quiet = 0;
            } else {
                quiet += 1;
            }
        }
        out
    }

    fn seq_of(id: usize, beats: &[f32], loops: u32, after: Option<usize>) -> Sequence {
        let mut at = 0.0;
        let mut steps = Vec::new();
        for &b in beats {
            steps.push(Step { at, len: 0.1, note: Note::new(0, 440.0, 0.0, 1.0) });
            at += b;
        }
        Sequence {
            id,
            steps,
            length: at,
            loops,
            channel: 0,
            at: None,
            grid: None,
            after,
            choke: false,
        }
    }

    /// A counted loop plays its pattern that many times, each pass on the beat,
    /// and then stops. 120bpm: a beat is 24000 frames.
    #[test]
    fn a_counted_sequence_loops_then_stops() {
        let hits = onsets(
            vec![SoundCommand::SetBpm(120.0), SoundCommand::Sequence(seq_of(1, &[1.0, 1.0], 3, None))],
            48000 * 4,
        );
        assert_eq!(hits.len(), 6, "2 steps x 3 passes, got {:?}", hits);
        for (k, &h) in hits.iter().enumerate() {
            let want = k * 24000;
            assert!(h >= want && h < want + 100, "hit {} at {}, want ~{}", k, h, want);
        }
    }

    /// `after` starts a sequence on the beat its predecessor's last pass ends.
    #[test]
    fn a_chained_sequence_starts_when_the_first_ends() {
        let hits = onsets(
            vec![
                SoundCommand::SetBpm(120.0),
                SoundCommand::Sequence(seq_of(1, &[1.0], 2, None)),
                SoundCommand::Sequence(seq_of(2, &[1.0], 1, Some(1))),
            ],
            48000 * 3,
        );
        assert_eq!(hits.len(), 3, "got {:?}", hits);
        assert!((48000..48100).contains(&hits[2]), "chained start at {}", hits[2]);
    }

    /// An infinite loop keeps going until stopped.
    #[test]
    fn a_forever_sequence_stops_on_request() {
        let (tx, rx) = channel::<SoundCommand>();
        tx.send(SoundCommand::SetBpm(240.0)).unwrap();
        tx.send(SoundCommand::Sequence(seq_of(7, &[1.0], 0, None))).unwrap();
        let mut mixer = make_mixer_boxed(48000.0, rx);
        let mut loud_late = 0.0f32;
        for i in 0..48000 * 2 {
            let v = mixer().abs();
            if i > 48000 + 2000 {
                loud_late = loud_late.max(v);
            }
        }
        assert!(loud_late > 0.01, "forever loop went quiet");
        tx.send(SoundCommand::StopSequence(Some(7))).unwrap();
        let mut after = 0.0f32;
        for i in 0..48000 {
            let v = mixer().abs();
            if i > 6000 {
                after = after.max(v);
            }
        }
        assert!(after < 0.001, "stopped loop still sounding: {}", after);
    }

    /// A bound slice plays only its part of the file: a slice of the silent half
    /// is silent.
    #[test]
    fn a_sample_slice_plays_only_its_range() {
        let mut pcm = vec![0.0f32; 8000];
        for (i, s) in pcm.iter_mut().enumerate().take(4000) {
            *s = (i as f32 * 0.05).sin();
        }
        let peak = peak_of(
            vec![
                SoundCommand::LoadSample("half".into(), pcm, 48000.0),
                SoundCommand::BindSample(3, "half".into(), None, Envelope::default(), (0.5, 1.0)),
                SoundCommand::PlayNote(Note::new(3, 440.0, 1.0, 1.0), None),
            ],
            8000,
        );
        assert!(peak < 0.001, "silent slice made sound: {}", peak);
    }

    /// The most basic contract there is: `note()` must make sound. Guards the
    /// whole chain (channel/lane allocation, envelope, oscillator, effect chain,
    /// master limiter) against silent regressions.
    #[test]
    fn plain_note_makes_sound() {
        let peak = peak_of(
            vec![SoundCommand::PlayNote(
                Note::new(0, 440.0, 1.0, 1.0),
                None,
            )],
            4800, // 100ms
        );
        assert!(peak > 0.01, "expected signal, got peak {}", peak);
    }

    /// Explicit channels must sound too (the default is channel 0).
    #[test]
    fn note_on_explicit_channel_makes_sound() {
        let peak = peak_of(
            vec![SoundCommand::PlayNote(
                Note::new(0, 440.0, 1.0, 1.0),
                Some(3),
            )],
            4800,
        );
        assert!(peak > 0.01, "expected signal, got peak {}", peak);
    }

    /// A decaying sine, standing in for a loaded ogg.
    fn fake_pcm(n: usize) -> Vec<f32> {
        (0..n)
            .map(|i| {
                let t = i as f32 / n as f32;
                (i as f32 * 0.06).sin() * (1.0 - t)
            })
            .collect()
    }

    /// What the `sounder` piano actually does: bind a loaded sound into a slot and
    /// play notes through it on an explicit channel. Covers the sampler path, which
    /// the oscillator tests above don't touch.
    #[test]
    fn bound_sample_makes_sound() {
        let peak = peak_of(
            vec![
                SoundCommand::LoadSample("tone".into(), fake_pcm(22050), 44100.0),
                SoundCommand::BindSample(2, "tone".into(), None, Envelope::default(), (0.0, 1.0)),
                SoundCommand::PlayNote(Note::new(2, 440.0, 0.5, 1.0), Some(2)),
            ],
            4800,
        );
        assert!(peak > 0.01, "expected signal, got peak {}", peak);
    }

    /// Same, but with the bitcrush the piano applies to that channel. Extreme
    /// settings must still pass signal, not mute it.
    #[test]
    fn crushed_channel_still_makes_sound() {
        let peak = peak_of(
            vec![
                SoundCommand::LoadSample("tone".into(), fake_pcm(22050), 44100.0),
                SoundCommand::BindSample(2, "tone".into(), None, Envelope::default(), (0.0, 1.0)),
                SoundCommand::CrushChannel(2, 4.0, 600.0),
                SoundCommand::PlayNote(Note::new(2, 440.0, 0.5, 1.0), Some(2)),
            ],
            4800,
        );
        assert!(peak > 0.01, "expected signal, got peak {}", peak);
    }

    /// A reload sends Reset before the new app configures anything; notes after it
    /// must still sound.
    #[test]
    fn note_after_reset_makes_sound() {
        let peak = peak_of(
            vec![
                SoundCommand::Reset,
                SoundCommand::PlayNote(Note::new(0, 440.0, 1.0, 1.0), Some(2)),
            ],
            4800,
        );
        assert!(peak > 0.01, "expected signal, got peak {}", peak);
    }

    /// A sung note must sound, including its consonants. The onset/coda clusters
    /// went from `Vec` to a fixed-size `Cluster` to keep the audio thread away from
    /// the allocator, and nothing else covered the singing path — "sun" exercises an
    /// unvoiced onset burst, a vowel, and a voiced coda glide in one note.
    #[test]
    fn sung_note_makes_sound() {
        let syllable = crate::vocaloid::parse_syllable("sun");
        assert!(!syllable.onset.is_empty(), "expected an /s/ onset burst");
        let peak = peak_of(
            vec![SoundCommand::PlayNote(
                Note::sung(220.0, 1.0, 1.0, &syllable),
                None,
            )],
            9600, // 200ms — long enough to get past the onset into the vowel
        );
        assert!(peak > 0.01, "expected signal, got peak {}", peak);
    }

    /// A cluster longer than `Cluster::MAX` must truncate, not panic or corrupt:
    /// the voice indexes bursts by position, so an over-long word has to stay
    /// in bounds. "strengths" is the nastiest thing English offers here.
    #[test]
    fn overlong_cluster_truncates_and_still_sounds() {
        let syllable = crate::vocaloid::parse_syllable("strengths");
        assert!(
            syllable.onset.len() <= Cluster::MAX && syllable.coda.len() <= Cluster::MAX,
            "clusters must be capped at {}",
            Cluster::MAX
        );
        let peak = peak_of(
            vec![SoundCommand::PlayNote(
                Note::sung(220.0, 1.0, 1.0, &syllable),
                None,
            )],
            9600,
        );
        assert!(peak > 0.01, "expected signal, got peak {}", peak);
    }

    /// Render a sung syllable on its own and return the samples.
    fn sing_alone(syl: &str, hz: f32, secs: f32, n: usize) -> Vec<f32> {
        let (tx, rx) = channel::<SoundCommand>();
        let s = crate::vocaloid::parse_syllable(syl);
        tx.send(SoundCommand::PlayNote(Note::sung(hz, secs, 1.0, &s), Some(1)))
            .unwrap();
        let mut mixer = make_mixer_boxed(48000.0, rx);
        (0..n).map(|_| mixer()).collect()
    }

    fn rms(x: &[f32]) -> f32 {
        (x.iter().map(|v| v * v).sum::<f32>() / x.len() as f32).sqrt()
    }

    /// Every vowel at every pitch comes out equally loud. Before the bank was
    /// levelled, /u/ was 10× louder at 330 Hz than at 520 Hz, so a melody pumped
    /// and its high notes went quiet enough for the consonants and breath to
    /// read as a whisper.
    #[test]
    fn vowels_are_equally_loud_at_any_pitch() {
        let mut levels = Vec::new();
        for v in ["a", "ee", "oo", "o", "ae", "uh"] {
            for hz in [130.0, 220.0, 330.0, 520.0, 700.0] {
                let x = sing_alone(v, hz, 0.6, 24000);
                levels.push((v, hz, rms(&x[9600..24000])));
            }
        }
        let lo = levels.iter().map(|l| l.2).fold(f32::MAX, f32::min);
        let hi = levels.iter().map(|l| l.2).fold(0.0, f32::max);
        assert!(lo > 0.01, "a vowel went silent: {:?}", levels);
        assert!(hi / lo < 1.2, "vowel levels spread {:.2}x: {:?}", hi / lo, levels);
    }

    /// A consonant is quieter than the vowel it leads into (real speech puts an
    /// /s/ 10–15 dB down). At equal loudness every syllable starts with a hiss.
    #[test]
    fn consonants_sit_under_the_vowel() {
        for c in ["s", "f", "h", "t", "k", "p", "j", "sh"] {
            let x = sing_alone(&format!("{}a", c), 220.0, 0.4, 19200);
            let burst = rms(&x[..480]);
            let vowel = rms(&x[9600..16800]);
            assert!(
                burst < vowel * 0.45,
                "'{}' burst {:.3} vs vowel {:.3}",
                c,
                burst,
                vowel
            );
        }
    }
}

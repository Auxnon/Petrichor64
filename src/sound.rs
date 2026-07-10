use std::{
    collections::VecDeque,
    sync::mpsc::{channel, Receiver, Sender},
};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
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

    let device = if opt.device == "default" {
        host.default_output_device()
    } else {
        host.output_devices()?
            .find(|x| x.name().map(|y| y == opt.device).unwrap_or(false))
    }
    .expect("failed to find output device");
    println!("Output device: {}", device.name()?);

    let config = device.default_output_config().unwrap();
    println!("Default output config: {:?}", config);

    match config.sample_format() {
        cpal::SampleFormat::F32 => run::<f32>(&device, &config.into(), audience),
        cpal::SampleFormat::I16 => run::<i16>(&device, &config.into(), audience),
        cpal::SampleFormat::U16 => run::<u16>(&device, &config.into(), audience),
        // cpal 0.16 added many more sample formats; render f32 into whatever the
        // device wants where we can, else bail with a clear error.
        cpal::SampleFormat::I32 => run::<i32>(&device, &config.into(), audience),
        cpal::SampleFormat::U32 => run::<u32>(&device, &config.into(), audience),
        cpal::SampleFormat::F64 => run::<f64>(&device, &config.into(), audience),
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

    // Envelope ramp rates (per sample): ~4ms attack, ~12ms release. A real
    // envelope removes the clicks and the "notes fall off / merge poorly"
    // artifacts the old crude one-deep crossfade produced.
    let attack_rate = 1.0 / (0.004 * sample_rate);
    let release_rate = 1.0 / (0.012 * sample_rate);
    let master_volume = 0.2;

    // Default instrument: odd-harmonic (square-ish) additive tone. Harmonics are
    // small integers now (index i => harmonic i+1) so a per-voice phase wrapped
    // to [0, 2π) keeps full f32 precision — the old model multiplied phase by
    // frequencies like 440, which only worked via an ever-growing global clock
    // whose f32 precision decayed into the "scrambled after a while" bug.
    let default_instr = Instrument::new(
        usize::MAX,
        (1..=15)
            .map(|k| if k % 2 == 1 { 1.0 / k as f32 } else { 0.0 })
            .collect(),
        false,
    );
    let mut instruments: FxHashMap<usize, Instrument> = FxHashMap::default();

    // Per-channel state: `current` is the sounding voice, `fading` is a
    // just-released voice still ramping down so consecutive notes cross-fade
    // instead of clicking.
    let mut queues: [VecDeque<Note>; NUM_CH] = Default::default();
    let mut current: [Option<Voice>; NUM_CH] = Default::default();
    let mut fading: [Option<Voice>; NUM_CH] = Default::default();

    let mut next_value = move || -> f32 {
        // Drain every pending command each sample so triggers are effectively
        // sample-accurate (the old code polled once per ~2000 samples, which
        // quantised note timing and could drop/merge fast notes).
        while let Ok(cmd) = audience.try_recv() {
            match cmd {
                SoundCommand::PlayNote(note, ch) => {
                    queues[ch.unwrap_or(0).min(NUM_CH - 1)].push_back(note);
                }
                SoundCommand::Chain(notes, ch) => {
                    queues[ch.unwrap_or(0).min(NUM_CH - 1)].extend(notes);
                }
                SoundCommand::MakeInstrument(inst) => {
                    instruments.insert(inst.name, inst.normalized());
                }
                SoundCommand::Stop(ch) => {
                    let c = ch.min(NUM_CH - 1);
                    queues[c].clear();
                    // Release rather than hard-cut, so stopping doesn't click.
                    if let Some(v) = current[c].take() {
                        fading[c] = Some(v);
                    }
                }
                SoundCommand::FadeChannel(ch, _dur) => {
                    let c = ch.min(NUM_CH - 1);
                    if let Some(v) = current[c].take() {
                        fading[c] = Some(v);
                    }
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

            // Sounding voice: advance phase, attack toward full, sustain, then
            // hand off to `fading` for release when its duration elapses.
            if let Some(v) = current[c].as_mut() {
                let instr = instruments.get(&v.instrument).unwrap_or(&default_instr);
                v.advance();
                if v.env < 1.0 {
                    v.env = (v.env + attack_rate).min(1.0);
                }
                mix += synth(v.phase, instr) * v.volume * v.env;
                v.remaining -= 1.0 / sample_rate;
                if v.remaining <= 0.0 {
                    fading[c] = current[c].take();
                }
            }

            // Fading voice: release ramp to silence.
            if let Some(v) = fading[c].as_mut() {
                let instr = instruments.get(&v.instrument).unwrap_or(&default_instr);
                v.advance();
                v.env -= release_rate;
                if v.env <= 0.0 {
                    fading[c] = None;
                } else {
                    mix += synth(v.phase, instr) * v.volume * v.env;
                }
            }
        }

        (mix * master_volume).clamp(-1.0, 1.0)
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
fn synth(phase: f32, instr: &Instrument) -> f32 {
    let mut total = 0.0;
    for (i, amp) in instr.freqs.iter().enumerate() {
        if *amp != 0.0 {
            total += (phase * (i + 1) as f32).sin() * amp;
        }
    }
    total / instr.divisor
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

#[derive(Clone, Copy)]
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

/// Runtime state for one sounding note: a wrapped phase accumulator plus a
/// linear envelope. Kept separate from `Note` (the queued spec).
struct Voice {
    phase: f32,
    phase_inc: f32,
    instrument: usize,
    remaining: f32,
    volume: f32,
    env: f32,
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
        }
    }
    /// Advance and wrap the phase to keep f32 precision indefinitely.
    fn advance(&mut self) {
        self.phase += self.phase_inc;
        if self.phase >= TWO_PI {
            self.phase -= TWO_PI;
        }
    }
}

pub struct Instrument {
    name: usize,
    /// Harmonic amplitudes; index i is harmonic (i+1) of the fundamental.
    freqs: Vec<f32>,
    divisor: f32,
}

impl Instrument {
    pub fn new(name: usize, freqs: Vec<f32>, _half: bool) -> Self {
        let divisor = Self::compute_divisor(&freqs);
        Self {
            name,
            freqs,
            divisor,
        }
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

pub enum SoundCommand {
    MakeInstrument(Instrument),
    PlayNote(Note, Option<usize>),
    Chain(Vec<Note>, Option<usize>),
    Stop(usize),
    FadeChannel(usize, f32),
}

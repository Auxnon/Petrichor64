use std::sync::mpsc::{channel, Receiver, Sender};

//use byte_slice_cast::AsByteSlice;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

pub type SoundPacket = (f32, f32, Vec<f32>, Vec<f32>);

pub struct Note {
    pub frequency: f32,
    pub duration: f32,
    pub volume: f32,
    pub channel: usize,
    pub func: Box<dyn Fn(f32) -> f32>,
}

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
        //let app = clap::App::new("beep").arg_from_usage("[DEVICE] 'The audio device to use'");
        #[cfg(all(
            any(target_os = "linux", target_os = "dragonfly", target_os = "freebsd"),
            feature = "jack"
        ))]
        //let app = app.arg_from_usage("-j, --jack 'Use the JACK host");
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

pub fn init() -> (anyhow::Result<cpal::Stream>, Sender<SoundPacket>) {
    let (singer, audience) = channel::<SoundPacket>();
    (init_sound(audience), singer)
}

pub fn init_sound(audience: Receiver<SoundPacket>) -> anyhow::Result<cpal::Stream> {
    let opt = Opt::from_args();

    // Conditionally compile with jack if the feature is specified.
    #[cfg(all(
        any(target_os = "linux", target_os = "dragonfly", target_os = "freebsd"),
        feature = "jack"
    ))]
    // Manually check for flags. Can be passed through cargo with -- e.g.
    // cargo run --release --example beep --features jack -- --jack
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
    }
}

pub fn run<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    audience: Receiver<SoundPacket>,
) -> Result<cpal::Stream, anyhow::Error>
where
    T: cpal::Sample,
{
    let sample_rate = config.sample_rate.0 as f32;
    println!("Sample rate: {}", sample_rate);
    let channels = config.channels as usize;

    // Produce a sinusoid of maximum amplitude.
    let mut sample_clock = 0f64;
    // let mut next_value = move || {
    //     sample_clock = (sample_clock + 1.0) % sample_rate;
    //     //println!("sample {}", sample_clock);
    //     (sample_clock * 440.0 * 2.0 * std::f32::consts::PI / sample_rate).sin()
    // };

    // for b in buffer.as_byte_slice() {
    //     println!("buffer {}", b);
    // }
    let volume_speed = 0.001;
    let fade_speed = 0.00005;
    // let mut vol = 1.;
    let mut occupied = [false; 16];
    let mut buffer = vec![];
    let mut once = false;
    let mut buffer_count = 1;
    let mut buffer_sum = 0.;

    let mut notes = vec![];

    //DEV we divide our harmonic count by log(n) to bring the volume down to base amplitude, hopefully.
    let flat = |a: f32| a.sin();
    let lowsquare = |a: f32| {
        let mut total = 0.;
        for i in (1..17).step_by(2) {
            total += (i as f32 * a).sin() / (i as f32)
        }
        //log 17
        total / 2.83
    };
    let square = |a: f32| {
        let mut total = 0.;
        for i in (1..63).step_by(2) {
            total += (i as f32 * a).sin() / (i as f32)
        }
        //log 63 = 4.143134726391533
        total / 4.143134726391533
    };
    let triangle = |a: f32| {
        let mut total = 0.;
        for i in 1..17 {
            total += (i as f32 * a).sin() / (i as f32)
        }
        total
    };
    let wave = |a: f32| {
        let mut total = 0.;
        for i in (1..64).step_by(16) {
            total += (i as f32 * a).sin() / (i as f32)
        }
        total
    };

    let noise = |a: f32| {
        let mut total = 0.;
        for i in 1..17 {
            total += rand::random::<f32>() * 2. - 1.
        }
        total
    };

    let saw = |a: f32| {
        a.sin() - (2. * a).sin() / 2. + (3. * a).sin() / 3. - (4. * a).sin() / 4.
            + (5. * a).sin() / 5.
    };

    let triangle = |a: f32| {
        let mut total = 0.;
        for i in 1..17 {
            total += (i as f32 * a).sin() / (i as f32)
        }
        total
    };

    let flute1 = |a: f32| {
        let mut total = 0.;
        for (i, n) in [1., 0.65, 0.61, 0.15, 0.09, 0.02, 0.02, 0.01, 0.01, 0.01, 0.]
            .iter()
            .enumerate()
        {
            total += ((i + 1) as f32 * a).sin() * n;
        }
        total / 2.397
    };
    let flute = |a: f32| {
        let mut total = 0.;
        for (i, n) in [1., 0.61, 0.1, 0.24, 0.11, 0.09, 0.0, 0.02, 0.0, 0.0, 0.1]
            .iter()
            .enumerate()
        {
            total += ((i + 1) as f32 * a).sin() * n;
        }
        total / 2.397
    };

    let mut instrument: Vec<f32> = vec![];
    let mut amps: Vec<f32> = vec![];
    let mut instrument_diviser = 1.;

    let musician = |a: f32, inst: &Vec<f32>, amp: &Vec<f32>, divisor: f32| {
        let mut total = 0.;
        for (i, n) in inst.iter().enumerate() {
            total += (n * a).sin(); // * amp[i];
        }
        total / divisor
    };

    let mut func = musician;
    let mut next_value = move || {
        sample_clock = (sample_clock + 1.0); // % sample_rate as f64;

        // println!("clock {}", sample_clock);
        //let b = buffer.as_chunks()
        // println!("byte {}", b.len());
        //let f = [(sample_clock) as usize] as f32

        match audience.try_recv() {
            Ok(packet) => {
                if packet.0 < 0. {
                    if !once {
                        // MARK sound save
                        // println!("save sound");
                        // crate::texture::save_audio_buffer(&buffer);
                        // buffer.clear();
                        // once = true;
                    }
                    instrument = packet.2;
                    amps = packet.3;
                    instrument_diviser = (instrument.len() as f32).ln();

                    let channel = match occupied.iter().position(|x| *x == false) {
                        Some(i) => i,
                        None => 0,
                    };
                    println!(
                        "channel {} and volume divisor {} from length {}",
                        channel,
                        instrument_diviser,
                        instrument.len()
                    );
                    occupied[channel] = true;
                    // note, timer, current_level aka volume, channel
                    notes.push((1., packet.1, 0., channel));

                    // func = musician;
                } else {
                    let channel = match occupied.iter().position(|x| *x == false) {
                        Some(i) => i,
                        None => 0,
                    };
                    println!("channel {}", channel);
                    occupied[channel] = true;
                    // note, timer, current_level aka volume, channel
                    notes.push((packet.0, packet.1, 0., channel));
                }
                // println!("notes {:?}", notes);
            }
            _ => {}
        };

        let mut all_waves = 0.;
        let master_volume = 0.5;

        // sample rate is 44100

        let float_clock = ((sample_clock) / sample_rate as f64) as f32;
        // we divide by 440. as our notes will mukltiply by a factor of this base frequency, note A is 440. and our frequency would be equal to base
        let cycler = (float_clock) * 2.0 * std::f32::consts::PI; //+ note.3 as f32
                                                                 //
        notes.retain_mut(|note| {
            // println!("note {}", note.0);
            // value += note.0;
            note.1 -= fade_speed;

            let volume = note.2;

            let totes = cycler * note.0;
            all_waves +=
                func(totes, &instrument, &amps, instrument_diviser) * volume * master_volume;

            //   square(totes)* volume * master_volume;

            if note.1 > 0. {
                if note.2 < 1. {
                    note.2 += volume_speed;
                }
                true
            } else {
                if note.2 > 0. {
                    note.2 -= volume_speed;
                    true
                } else {
                    occupied[note.3] = false;
                    false
                }
            }
        });
        // all_waves = all_waves.clamp(-1., 1.);

        // ((bufferIn[(2 * sample_clock as usize)]) as f32) / 128.
        // let cycler = sample_clock * 2.0 * std::f32::consts::PI / sample_rate;

        //*440.//println!("t {}", t);

        // switch_board.write().h = t;

        //sine
        //(t).sin()

        // let a = 2.;
        // let p = 4.;

        //triangle
        /*

        (4. * a / p) * (((t - p / 4.) % p) - p / 2.).abs() - a
        */
        //square

        // if (t * 1.8) % std::f32::consts::PI > 1. {
        //     1.
        // } else {
        //     0.
        // }

        //cool small fourier series
        //t.sin() - (2. * t).sin() / 2.

        //sawtooth
        // t.sin() - (2. * t).sin() / 2. + (3. * t).sin() / 3. - (4. * t).sin() / 4.
        //     + (5. * t).sin() / 5.;

        // const pi: f32 = std::f32::consts::PI;
        // -0.25 * (3. * t * pi).sin() + 0.25 * (pi * t).sin() + (t * pi).cos() * 3f32.sqrt() / 2.

        // let s = pia(freq, t, 1., 1.)
        //     + pia(freq, t, 2., 2.)
        //     + pia(freq, t, 3., 4.)
        //     + pia(freq, t, 4., 8.)
        //     + pia(freq, t, 5., 16.)
        //     + pia(freq, t, 6., 32.);
        // s * s * s

        // let hertz = value;

        // let totes = cycler * hertz;

        // square(totes)
        buffer_sum += all_waves * 256.;

        if buffer_count >= 64 {
            buffer.push((buffer_sum / 64. + 256.) as u8);
            buffer_sum = 0.;
            buffer_count = 1;
        }
        buffer_count += 1;

        all_waves
    };

    let err_fn = |err| eprintln!("an error occurred on stream: {}", err);

    let stream = device.build_output_stream(
        config,
        move |data: &mut [T], _: &cpal::OutputCallbackInfo| {
            write_data(data, channels, &mut next_value)
        },
        err_fn,
    )?;
    //td::thread::spawn(f)
    stream.play()?;
    Ok(stream)

    //std::thread::sleep(std::time::Duration::from_millis(3000));

    //Ok(())
}

fn pia(freq: f32, t: f32, a: f32, b: f32) -> f32 {
    let frequency = freq;
    (a * 2. * std::f32::consts::PI * frequency * t).sin()
        * (-0.0004 * 2. * std::f32::consts::PI * frequency * t).exp()
        / b
}
fn write_data<T>(output: &mut [T], channels: usize, next_sample: &mut dyn FnMut() -> f32)
where
    T: cpal::Sample,
{
    for frame in output.chunks_mut(channels) {
        let value: T = cpal::Sample::from::<f32>(&next_sample());
        for sample in frame.iter_mut() {
            *sample = value;
        }
    }
}

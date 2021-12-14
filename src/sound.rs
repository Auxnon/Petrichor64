use std::sync::{Arc, Mutex};

//use byte_slice_cast::AsByteSlice;
use cpal::{
    traits::{DeviceTrait, HostTrait, StreamTrait},
    Sample,
};
use parking_lot::RwLock;

use crate::switch_board::SwitchBoard;

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

pub fn init_sound(switch_board: Arc<RwLock<SwitchBoard>>) -> anyhow::Result<cpal::Stream> {
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
        cpal::SampleFormat::F32 => run::<f32>(&device, &config.into(), switch_board),
        cpal::SampleFormat::I16 => run::<i16>(&device, &config.into(), switch_board),
        cpal::SampleFormat::U16 => run::<u16>(&device, &config.into(), switch_board),
    }
}

pub fn run<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    switch_board: Arc<RwLock<SwitchBoard>>,
) -> Result<cpal::Stream, anyhow::Error>
where
    T: cpal::Sample,
{
    let sample_rate = config.sample_rate.0 as f32;
    let channels = config.channels as usize;

    // Produce a sinusoid of maximum amplitude.
    let mut sample_clock = 0f32;
    // let mut next_value = move || {
    //     sample_clock = (sample_clock + 1.0) % sample_rate;
    //     //println!("sample {}", sample_clock);
    //     (sample_clock * 440.0 * 2.0 * std::f32::consts::PI / sample_rate).sin()
    // };

    // for b in buffer.as_byte_slice() {
    //     println!("buffer {}", b);
    // }
    let mut i = 6f32;

    let mut next_value = move || {
        sample_clock = (sample_clock + 1.0) % sample_rate;

        //println!("ready");
        //let b = buffer.as_chunks()
        // println!("byte {}", b.len());
        //let f = [(sample_clock) as usize] as f32

        let mut level = if switch_board.read().space { 6.0 } else { 12.0 };
        i += ((level - i) / sample_rate);

        let freq = i;
        //println!("{}", i);
        // + t / 2000.;
        // match read().unwrap() {
        //     //i think this speaks for itself
        //     Event::Key(KeyEvent {
        //         code: KeyCode::Char('h'),
        //         modifiers: KeyModifiers::CONTROL,
        //         //clearing the screen and printing our message
        //     }) => {
        //         //execute!(stdout, Clear(ClearType::All), Print("Hello world!")).unwrap()
        //         freq = 1.;
        //     }
        //     // Event::Key(KeyEvent {
        //     //     code: KeyCode::Char('t'),
        //     //     modifiers: KeyModifiers::ALT,
        //     // }) => execute!(stdout, Clear(ClearType::All), Print("crossterm is cool")).unwrap(),
        //     Event::Key(KeyEvent {
        //         code: KeyCode::Char('q'),
        //         modifiers: KeyModifiers::CONTROL,
        //     }) => {}
        //     _ => (),
        // }

        // ((bufferIn[(2 * sample_clock as usize)]) as f32) / 128.
        let t = sample_clock * 2.0 * std::f32::consts::PI / sample_rate; //*440.
                                                                         //println!("t {}", t);

        switch_board.write().h = t;
        //sine
        //(t).sin()

        let a = 2.;
        let p = 4.;

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
        (t * std::f32::consts::PI * freq).sin() / 20.
        //(freq * t / 400.).sin() / 20.
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

use std::{sync::mpsc::{channel, Receiver, Sender}, collections::VecDeque};

//use byte_slice_cast::AsByteSlice;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use rustc_hash::FxHashMap;

use crate::sound;

pub type SoundPacket = (f32, f32, Vec<f32>, Vec<f32>);

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
    audience: Receiver<SoundCommand>,
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
    // if duration is less than this then start lowering volume by volume_speed
    let volume_fade_threshold=0.;//(1.0/volume_speed)*fade_speed;
    // let mut vol = 1.;
    // let mut occupied = [false; 16];
    let mut sound_channels:[VecDeque<Note>;16] = Default::default();
    let mut current_notes:[Option<Note>;16]=[None;16];
    let mut last_notes:[Option<Note>;16]=[None;16];
    let mut buffer = vec![];
    let mut once = false;
    let mut record = false;
    let mut buffer_count = 1;
    let mut buffer_sum = 0.;


    // |x| if x.sin()>0. {1.} else {-1.};
    let sqr_wave=Vec::from_iter((1..63 as usize).map(|x| if x%2==0 {440.} else {0.}));
    let square_wave= Instrument::new(99,sqr_wave,true);

    // let mut notes = vec![];

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
    // BLUE square
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

    let mut instruments: FxHashMap<usize,Instrument> = FxHashMap::default();
    let mut amps: Vec<f32> = vec![];
    let mut instrument_diviser = 1.;

    let mut checker = 0;
    let mut last_amp = 0.;

    //DEV whether to mult by ii in iteration and then div, or just div.

    //BLUE instrument
    let musician = |a: f32, instr:&Instrument| { //amp: &Vec<f32>,
        let mut total = 0.;
        for (i, n) in instr.freqs.iter().enumerate() {
            let ii = (i + 1) as f32;
            total += (a * n * ii).sin() / ii; // * amp[i];
        }
        total / instr.divisor
    };

    // let musician_cumulative = |a: f32, inst: &Vec<f32>, amp: &Vec<f32>, divisor: f32| {
    //     let mut total = 0.;
    //     for (i, n) in inst.iter().enumerate() {
    //         let ii = (i + 1) as f32;
    //         total += (a * n * ii).sin() / ii; // * amp[i];
    //     }
    //     total / divisor
    // };

    let mut func = musician;
    let mut audience_countdown=0;
    let mut next_value = move || {
        sample_clock = (sample_clock + 1.0); // % sample_rate as f64;

        // println!("clock {}", sample_clock);
        //let b = buffer.as_chunks()
        // println!("byte {}", b.len());
        //let f = [(sample_clock) as usize] as f32

        if audience_countdown> 2000{
            audience_countdown=0;
        
        match audience.try_recv() {
            Ok(packet) => {
                match packet {
                    SoundCommand::PlayNote(note,ichannel) => {

                        // occupied[note as usize] = true;


                        // let channel = match ichannel {
                        //     Some(u)=>u,
                        //     None=> match occupied.iter().position(|x| *x == false) {
                        //     Some(i) => i,
                        //     None => 0,
                        // }};
                        let channel = 0;


                        // note, timer, current_level aka volume, channel
                        sound_channels[channel].push_back(note);
                        // notes.push((packet.0, packet.1, 0., channel));
                    }
                    SoundCommand::Chain(notes,ichannel ) => {
                        println!("chain {}",notes.len());
                        let channel=0;
                        sound_channels[channel].extend(notes);
                       
                        // for note in notes {
                        //     sound_channels[channel].push(note);
                        // }
                        
                    }
                  
                    SoundCommand::MakeInstrument(mut inst) => {
                        // println!("instrument {}", inst);
                        // instrument = inst;
                        // instrument_diviser = 0.;
                        // for (i, n) in instrument.iter().enumerate() {
                        //     let ii = (i + 1) as f32;
                        //     instrument_diviser += n * ii;
                        // }
                        // instrument_diviser = 1. / instrument_diviser;

                        inst.divisor = (inst.freqs.len() as f32).ln();

                        let div = *inst.freqs.get(0).unwrap_or(&1.);

                        inst.base_freq=if div != 0. {
                             1. / div
                        } else {
                             1.
                        };
                        // println!(
                        //     "channel {} and volume divisor {} from length {}",
                        //     channel,
                        //     instrument_diviser,
                        //     instrument.len()
                        // );


                        last_amp = *inst.freqs.get(0).unwrap_or(&0.);

                        instruments.insert(inst.name,inst);
                        // notes.push((1., packet.1, 0., channel));
                    }

                    SoundCommand::FadeChannel(ichannel,duration )=>{

                    }
                    SoundCommand::Stop(ichannel)=>{
                        sound_channels[ichannel].clear();   
                        current_notes[ichannel]=None;

                    }
                    // Packet::Wave(wave) => {
                    //     println!("wave {}", wave);
                    //     match wave {
                    //         Wave::Flat => func = musician,
                    //         Wave::Square => func = square,
                    //         Wave::Triangle => func = triangle,
                    //         Wave::Saw => func = saw,
                    //         Wave::Noise => func = noise,
                    //         Wave::Flute => func = flute,
                    //         Wave::Flute1 => func = flute1,
                    //         Wave::LowSquare => func = lowsquare,
                    //         Wave::Wave => func = wave,
                    //     }
                    // },

                    // Packet::Fade(fade) => {
                    //     println!("fade {}", fade);
                    // },
                    // Packet::VolumeSpeed(speed) => {
                    //     println!("volume speed {}", speed);
                    //     //volume_speed = speed;
                    // },
                }
              
                    // if !once && !record {
                    //     // record = true;
                    // } else if !once {
                    //     // DEV sound save
                    //     println!("save sound");
                    //     crate::texture::save_audio_buffer(&buffer);
                    //     buffer.clear();
                    //     // once = true;
                    // }
                  

            
            }
            _ => {
                // println!("no packet");
            }
        };
    }
    audience_countdown+=1;

        let mut all_waves = 0.;
        let mut master_volume = 0.1;

        // sample rate is 44100

        let float_clock = ((sample_clock) / sample_rate as f64) as f32;
        // we divide by 440. as our notes will mukltiply by a factor of this base frequency, note A is 440. and our frequency would be equal to base
        let cycler = (float_clock) * 2.0 * std::f32::consts::PI; //+ note.3 as f32

        // checker += 1;
        // if checker > 9000 {
        //     checker = 0;
        //     println!(
        //         "cycler {} clock {} last amp {}",
        //         cycler, sample_clock, last_amp
        //     );
        // }

        match &mut current_notes[0]{
            Some(note)=>{
                // note, timer, current_level aka volume, channel
                note.duration-=fade_speed;
                // note.volume
                let instr = instruments.get(&note.instrument).unwrap_or(&square_wave);


                let totes = cycler * note.frequency*instr.base_freq;

                match &mut last_notes[0]{
                    Some(last_note)=>{
                        let last_instr = instruments.get(&note.instrument).unwrap_or(&square_wave);
                        if last_note.instrument != note.instrument{
                            last_amp = 0.;
                        }
                        let last_totes = cycler * last_note.frequency*last_instr.base_freq;
                        let v1=last_note.volume;
                        let v2=1. -v1; 
                        all_waves += func(totes, instr) * note.volume * master_volume*v2 + func(last_totes, last_instr) * master_volume*v1;   
                        last_note.volume-=volume_speed;
                        if last_note.volume<=0.{
                            last_notes[0]=None;
                        }
                    },
                    None=>{
                        all_waves +=
                        // choose
                        // square(cycler * last_amp) * volume * master_volume;
                        func(totes, instr) * note.volume * master_volume;
                    }
                }
          

            //   square(totes)* volume * master_volume;

            
            // if note.1 > 0. {
            //     if note.2 < 1. {
            //         note.2 += volume_speed;
            //     }
            //     true
            // } else {
            //     if note.2 > 0. {
            //         note.2 -= volume_speed;
            //         true
            //     } else {
            //         occupied[note.3] = false;
            //         false
            //     }
            // }

                if note.duration <= 0. {
                    last_notes[0]=current_notes[0];
                    current_notes[0] = sound_channels[0].pop_front();
                    if let Some(n) = current_notes[0] {
                        print!("freq {} dur{} ", n.frequency,n.duration);
                    }
                   
                }else if note.duration <=volume_fade_threshold{
                    note.volume -= volume_speed;
                }
            }
            None=>{
                current_notes[0] = sound_channels[0].pop_front();
                if let Some(n) = current_notes[0] {
                    print!("freq {} dur{} ", n.frequency,n.duration);
                }
            }
        }

       

        let abs = all_waves.abs();
        if abs > 1. {
            master_volume = 0.1 / abs;
            all_waves /= abs;
        }
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

        if record {
            buffer_sum += all_waves * 256.;

            if buffer_count >= 64 {
                println!("buffer sum {}", all_waves);
                buffer.push((buffer_sum + 256.) as u8);
                buffer_sum = 0.;
                buffer_count = 1;
            }
            buffer_count += 1;
        }

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


    // DEV ????
    // std::thread::spawn(move || {
    //     stream.play()?;
    // });
    stream.play()?;
    // write the buffer to a file
    // let mut file = File::create("test.wav")?;
    // file.write_all(&buffer)?;

    // Ok(stream)

    //std::thread::sleep(std::time::Duration::from_millis(3000));

    Ok(stream)
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
// note, timer, current_level aka volume, channel
#[derive(Clone, Copy)]
pub struct Note {
    instrument: usize,
    pub frequency: f32,
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
pub struct Instrument {
    name: usize,
    freqs: Vec<f32>,
    half: bool,
    divisor: f32,
    base_freq: f32,
}

impl Instrument {
    pub fn new(name: usize, freqs: Vec<f32>, half: bool,) -> Self {
        Self {
            name,
            freqs,
            half,
            divisor: 1.,    
            base_freq: 1.,
        }
    }
}

pub enum SoundCommand {
    MakeInstrument(Instrument),
    PlayNote(Note, Option<usize>),
    Chain(Vec<Note>, Option<usize>),
    Stop(usize),
    // StopChannel(usize),
    FadeChannel(usize, f32),
}

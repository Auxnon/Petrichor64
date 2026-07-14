//! oggify — convert audio files to OGG Vorbis for Petrichor64's `sounds/` folder.
//!
//! Walks a directory (recursively), converts every `.wav`/`.mp3` it finds into a
//! sibling `.ogg`, then offers to delete the originals (default: no). Files that
//! are already `.ogg` are skipped.
//!
//! Kept as a separate workspace crate on purpose: the Vorbis *encoder*
//! (`vorbis_rs`) vendors libvorbis C source, which we don't want linked into the
//! size-optimized engine binary. The engine only *decodes* ogg (via `lewton`).
//!
//! Usage: `cargo run -p oggify -- <dir>`  (dir defaults to `.`)

use std::io::Write;
use std::num::{NonZeroU32, NonZeroU8};
use std::path::{Path, PathBuf};

use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::{DecoderOptions, CODEC_TYPE_NULL};
use symphonia::core::errors::Error as SymphoniaError;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;
use vorbis_rs::VorbisEncoderBuilder;
use walkdir::WalkDir;

fn main() {
    let dir = std::env::args().nth(1).unwrap_or_else(|| ".".to_string());
    let root = PathBuf::from(&dir);
    if !root.exists() {
        eprintln!("oggify: path does not exist: {}", root.display());
        std::process::exit(1);
    }

    // Collect convertible sources first so we can report and prompt at the end.
    let sources: Vec<PathBuf> = WalkDir::new(&root)
        .into_iter()
        .filter_map(Result::ok)
        .map(|e| e.into_path())
        .filter(|p| matches!(ext_lower(p).as_deref(), Some("wav") | Some("mp3")))
        .collect();

    if sources.is_empty() {
        println!("oggify: no .wav/.mp3 files found under {}", root.display());
        return;
    }

    println!("oggify: converting {} file(s) under {}", sources.len(), root.display());
    let mut converted: Vec<PathBuf> = Vec::new();
    for src in &sources {
        let out = src.with_extension("ogg");
        match convert(src, &out) {
            Ok(()) => {
                println!("  ✓ {} -> {}", src.display(), out.display());
                converted.push(src.clone());
            }
            Err(e) => eprintln!("  ✗ {}: {}", src.display(), e),
        }
    }

    if converted.is_empty() {
        println!("oggify: nothing converted.");
        return;
    }

    // Offer to delete the originals. Default is NO — an empty answer keeps them.
    print!(
        "\nDelete {} original file(s)? Their .ogg conversions remain. [y/N] ",
        converted.len()
    );
    let _ = std::io::stdout().flush();
    let mut answer = String::new();
    if std::io::stdin().read_line(&mut answer).is_ok()
        && matches!(answer.trim().to_lowercase().as_str(), "y" | "yes")
    {
        for src in &converted {
            match std::fs::remove_file(src) {
                Ok(()) => println!("  deleted {}", src.display()),
                Err(e) => eprintln!("  failed to delete {}: {}", src.display(), e),
            }
        }
    } else {
        println!("kept originals.");
    }
}

fn ext_lower(p: &Path) -> Option<String> {
    p.extension().and_then(|e| e.to_str()).map(|s| s.to_lowercase())
}

/// Decode `src` (wav/mp3) to planar f32 and encode it as OGG Vorbis at `out`.
fn convert(src: &Path, out: &Path) -> Result<(), String> {
    let (planar, sample_rate, channels) = decode(src)?;

    let sr = NonZeroU32::new(sample_rate).ok_or("zero sample rate")?;
    let ch = NonZeroU8::new(channels as u8).ok_or("zero channels")?;

    let file = std::fs::File::create(out).map_err(|e| format!("create {}: {}", out.display(), e))?;
    let mut encoder = VorbisEncoderBuilder::new(sr, ch, file)
        .map_err(|e| format!("encoder init: {}", e))?
        .build()
        .map_err(|e| format!("encoder build: {}", e))?;
    encoder
        .encode_audio_block(&planar)
        .map_err(|e| format!("encode: {}", e))?;
    encoder.finish().map_err(|e| format!("finish: {}", e))?;
    Ok(())
}

/// Decode any supported input to per-channel (planar) f32 samples.
fn decode(src: &Path) -> Result<(Vec<Vec<f32>>, u32, usize), String> {
    let file = std::fs::File::open(src).map_err(|e| format!("open: {}", e))?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());

    let mut hint = Hint::new();
    if let Some(ext) = ext_lower(src) {
        hint.with_extension(&ext);
    }

    let probed = symphonia::default::get_probe()
        .format(&hint, mss, &FormatOptions::default(), &MetadataOptions::default())
        .map_err(|e| format!("probe: {}", e))?;
    let mut format = probed.format;

    let track = format
        .tracks()
        .iter()
        .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
        .ok_or("no decodable audio track")?;
    let track_id = track.id;

    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &DecoderOptions::default())
        .map_err(|e| format!("decoder: {}", e))?;

    let mut planar: Vec<Vec<f32>> = Vec::new();
    let mut sample_rate = 0u32;
    let mut channels = 0usize;
    let mut sample_buf: Option<SampleBuffer<f32>> = None;

    loop {
        let packet = match format.next_packet() {
            Ok(p) => p,
            // End of stream (or a trailing read error): stop and keep what we have.
            Err(SymphoniaError::IoError(_)) => break,
            Err(e) => return Err(format!("packet: {}", e)),
        };
        if packet.track_id() != track_id {
            continue;
        }
        match decoder.decode(&packet) {
            Ok(decoded) => {
                let spec = *decoded.spec();
                if sample_buf.is_none() {
                    sample_rate = spec.rate;
                    channels = spec.channels.count().max(1);
                    planar = vec![Vec::new(); channels];
                    sample_buf = Some(SampleBuffer::<f32>::new(decoded.capacity() as u64, spec));
                }
                let sb = sample_buf.as_mut().unwrap();
                sb.copy_interleaved_ref(decoded);
                // De-interleave into per-channel buffers for the vorbis encoder.
                for (i, s) in sb.samples().iter().enumerate() {
                    planar[i % channels].push(*s);
                }
            }
            // Recoverable glitches: skip the packet and keep going.
            Err(SymphoniaError::DecodeError(_)) => continue,
            Err(SymphoniaError::IoError(_)) => break,
            Err(e) => return Err(format!("decode: {}", e)),
        }
    }

    if planar.is_empty() || planar[0].is_empty() {
        return Err("decoded no audio".to_string());
    }
    Ok((planar, sample_rate, channels))
}

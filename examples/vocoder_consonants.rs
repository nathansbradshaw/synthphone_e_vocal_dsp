//! Host-only real-vocal consonant audit for Vocode mode.
//!
//! Streams a mono/stereo WAV through the 1024-point stream entry point with a
//! held chord carrier built like the firmware's `render_midi_hop` (per-note
//! oscillators summed and divided by the active count), then compares the
//! level of unvoiced (consonant) frames against voiced (vowel) frames before
//! and after processing. A consonant balance near 0 dB means consonants stay
//! as prominent relative to vowels as in the original voice.
//!
//! cargo run --release --features std --example vocoder_consonants -- \
//!     VOCAL.wav OUT_DIR START_SECONDS DURATION_SECONDS [triangle|saw|square|sine] [mix 0..1]
use std::{error::Error, fs, path::Path};
use synthphone_e_vocal_dsp::{
    MusicalSettings, ProcessingMode, VocalEffectsConfig, VocalStreamState,
    audio::{Oscillator, Waveform},
    process_vocal_effects_1024_stream,
};

const N: usize = 1024;
const HOP: usize = 256;

fn read_mono(path: &Path) -> Result<(Vec<f32>, u32), Box<dyn Error>> {
    let mut reader = hound::WavReader::open(path)?;
    let spec = reader.spec();
    let channels = spec.channels as usize;
    let raw: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Float => reader.samples::<f32>().collect::<Result<_, _>>()?,
        hound::SampleFormat::Int => {
            let scale = 1.0 / (1u64 << (spec.bits_per_sample - 1)) as f32;
            reader
                .samples::<i32>()
                .map(|s| s.map(|v| v as f32 * scale))
                .collect::<Result<_, _>>()?
        }
    };
    let mono = raw.chunks(channels).map(|c| c.iter().sum::<f32>() / channels as f32).collect();
    Ok((mono, spec.sample_rate))
}

fn write(path: &Path, samples: &[f32], rate: u32) -> Result<(), Box<dyn Error>> {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: rate,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let mut writer = hound::WavWriter::create(path, spec)?;
    for &s in samples {
        writer.write_sample(s)?;
    }
    writer.finalize()?;
    Ok(())
}

/// Returns (total power, power above 3 kHz) per 1024-sample frame, hop 512.
fn band_powers(signal: &[f32], rate: f32) -> Vec<(f32, f32)> {
    use synthphone_e_vocal_dsp::dsp::{Fft1024, FftOps};
    let window = Fft1024::get_hann_window();
    let split = (3000.0 * N as f32 / rate) as usize;
    let mut out = Vec::new();
    let mut start = 0;
    while start + N <= signal.len() {
        let mut frame: [f32; N] = core::array::from_fn(|i| signal[start + i] * window[i]);
        let fft = Fft1024::forward_fft(&mut frame);
        let (mut total, mut high) = (0.0, 0.0);
        for (i, bin) in fft.iter().enumerate().skip(1) {
            let p = bin.re * bin.re + bin.im * bin.im;
            total += p;
            if i >= split {
                high += p;
            }
        }
        out.push((total, high));
        start += N / 2;
    }
    out
}

fn db(x: f32) -> f32 {
    10.0 * (x.max(1e-20)).log10()
}

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() < 4 {
        return Err("usage: VOCAL.wav OUT_DIR START DURATION [waveform] [mix]".into());
    }
    let (mono, rate) = read_mono(Path::new(&args[0]))?;
    let out_dir = Path::new(&args[1]);
    fs::create_dir_all(out_dir)?;
    let start = (args[2].parse::<f32>()? * rate as f32) as usize;
    let len = (args[3].parse::<f32>()? * rate as f32) as usize;
    let name = args.get(4).map(String::as_str).unwrap_or("triangle");
    let waveform = match name {
        "saw" => Waveform::Saw,
        "square" => Waveform::Square,
        "sine" => Waveform::Sine,
        _ => Waveform::Triangle,
    };
    let mix: f32 = args.get(5).map(|s| s.parse()).transpose()?.unwrap_or(1.0);
    let input = &mono[start.min(mono.len())..(start + len).min(mono.len())];

    // A-minor triad, like three held MIDI notes at full gain.
    let chord = [220.0, 261.63, 329.63];
    let mut oscillators = chord.map(|f| Oscillator::new(f, rate as f32, waveform));
    let carrier: Vec<f32> = (0..input.len())
        .map(|_| oscillators.iter_mut().map(|o| o.next_value()).sum::<f32>() / chord.len() as f32)
        .collect();

    let config =
        VocalEffectsConfig { sample_rate: rate as f32, vocoder_mix: mix, ..Default::default() };
    let mut midi_frequencies = [0.0; 8];
    let mut midi_gains = [0.0; 8];
    for (i, &f) in chord.iter().enumerate() {
        midi_frequencies[i] = f;
        midi_gains[i] = 1.0;
    }
    let settings = MusicalSettings {
        mode: ProcessingMode::Vocode,
        midi_frequencies,
        midi_gains,
        ..Default::default()
    };
    let mut state = VocalStreamState::new();
    let (mut in_ph, mut out_ph) = ([0.0; N], [0.0; N]);
    let (mut env, mut inv) = ([1.0; N / 2], [1.0; N / 2]);
    let mut output = vec![0.0f32; input.len()];
    let mut pos = 0;
    while pos + N <= input.len() {
        let mut frame: [f32; N] = core::array::from_fn(|i| input[pos + i]);
        let mut carrier_frame: [f32; N] = core::array::from_fn(|i| carrier[pos + i]);
        let result = process_vocal_effects_1024_stream(
            &mut frame,
            Some(&mut carrier_frame),
            &mut in_ph,
            &mut out_ph,
            &mut env,
            &mut inv,
            &mut state,
            &config,
            &settings,
        );
        // Hann^2 overlap-add at hop N/4 sums to 1.5.
        for i in 0..N {
            output[pos + i] += result[i] / 1.5;
        }
        pos += HOP;
    }

    let source = band_powers(input, rate as f32);
    let processed = band_powers(&output, rate as f32);
    let loudest = source.iter().map(|p| p.0).fold(0.0, f32::max);
    let (mut v_in, mut v_out, mut u_in, mut u_out, mut vn, mut un) = (0.0, 0.0, 0.0, 0.0, 0, 0);
    let (mut u_hf_in, mut u_hf_out, mut v_hf_in, mut v_hf_out) = (0.0, 0.0, 0.0, 0.0);
    for (&(total, high), &(out_total, out_high)) in source.iter().zip(&processed) {
        if total < loudest * 1e-4 {
            continue; // silence (< -40 dB re loudest frame)
        }
        let ratio = high / total;
        if ratio > 0.4 {
            u_in += total;
            u_out += out_total;
            u_hf_in += high;
            u_hf_out += out_high;
            un += 1;
        } else if ratio < 0.1 {
            v_in += total;
            v_out += out_total;
            v_hf_in += high;
            v_hf_out += out_high;
            vn += 1;
        }
    }
    let balance = (db(u_out / un as f32) - db(v_out / vn as f32))
        - (db(u_in / un as f32) - db(v_in / vn as f32));
    let report = format!(
        "rate {rate} Hz, waveform {name}, mix {mix}\n\
         voiced frames {vn}: output - input level {:+.1} dB, share above 3 kHz {:.2}% -> {:.2}%\n\
         unvoiced frames {un}: output - input level {:+.1} dB (above 3 kHz {:+.1} dB)\n\
         consonant balance vs original: {balance:+.1} dB\n",
        db(v_out) - db(v_in),
        100.0 * v_hf_in / v_in,
        100.0 * v_hf_out / v_out,
        db(u_out) - db(u_in),
        db(u_hf_out) - db(u_hf_in),
    );
    print!("{report}");
    let tag = format!("{name}-mix{mix}").to_lowercase();
    fs::write(out_dir.join(format!("report-{tag}.txt")), &report)?;
    write(&out_dir.join("input.wav"), input, rate)?;
    write(&out_dir.join(format!("vocoder-{tag}.wav")), &output, rate)?;
    Ok(())
}

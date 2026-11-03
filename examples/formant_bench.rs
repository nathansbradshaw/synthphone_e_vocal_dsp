//! Host-only relative benchmark against the preceding complex-IFFT algorithm.
//! These durations are not physical Daisy CPU cycle measurements.
use std::{hint::black_box, time::Instant};
use synthphone_e_vocal_dsp::dsp::{extract_cepstral_envelope, Fft1024, FftOps};

fn previous(magnitudes: &[f32; 512], envelope: &mut [f32; 512]) {
    let mut spectrum = [microfft::Complex32 { re: 0.0, im: 0.0 }; 1024];
    for i in 0..512 {
        spectrum[i].re = libm::logf(magnitudes[i].max(1e-6));
    }
    for i in 1..511 {
        spectrum[1024 - i].re = spectrum[i].re;
    }
    let cepstrum = Fft1024::inverse_fft(&mut spectrum);
    let mut liftered = [0.0; 1024];
    for i in 0..64 {
        liftered[i] = cepstrum[i].re;
    }
    for i in 961..1024 {
        liftered[i] = cepstrum[i].re;
    }
    let smooth = Fft1024::forward_fft(&mut liftered);
    for i in 0..512 {
        envelope[i] = libm::expf(smooth[i].re);
    }
}

fn main() {
    let magnitudes =
        core::array::from_fn(|i| 0.01 + 0.9 * (-((i as f32 - 20.0) / 8.0).powi(2)).exp());
    let mut old = [0.0; 512];
    let mut new = [0.0; 512];
    previous(&magnitudes, &mut old);
    extract_cepstral_envelope::<1024, 512, Fft1024>(&magnitudes, &mut new);
    let error = old.iter().zip(new).map(|(a, b)| (b / a - 1.0).abs()).fold(0.0_f32, f32::max);
    assert!(error < 0.000025, "envelope relative error {error}");
    let mut before = Vec::new();
    let mut after = Vec::new();
    for round in 0..10 {
        for current in [round % 2 == 0, round % 2 != 0] {
            let start = Instant::now();
            for _ in 0..1000 {
                if current {
                    extract_cepstral_envelope::<1024, 512, Fft1024>(
                        black_box(&magnitudes),
                        black_box(&mut new),
                    );
                } else {
                    previous(black_box(&magnitudes), black_box(&mut old));
                }
            }
            let micros = start.elapsed().as_secs_f64() * 1000.0;
            if current {
                after.push(micros);
            } else {
                before.push(micros);
            }
        }
    }
    before.sort_by(f64::total_cmp);
    after.sort_by(f64::total_cmp);
    println!("previous_median_us={}\nreal_fft_median_us={}\nspeedup={}\nmax_envelope_relative_error={error}\nHost relative benchmark only; physical board CPU timing remains unverified.", before[5], after[5], before[5] / after[5]);
}

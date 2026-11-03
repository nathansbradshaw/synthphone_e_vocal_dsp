//! Independent direct-DFT oracle and operation budget for the formant envelope.
use core::sync::atomic::{AtomicUsize, Ordering};
use synthphone_e_vocal_dsp::dsp::*;

fn check<const N: usize, const H: usize, F: FftOps<N, H>>() {
    for shape in 0..4 {
        let magnitudes = core::array::from_fn(|i| match shape {
            0 => 0.0,
            1 => {
                if i == 7 || i == H - 1 {
                    0.3
                } else {
                    0.00001
                }
            }
            2 => 0.01 + 0.8 * (-((i as f64 - 15.0) / 8.0).powi(2)).exp() as f32,
            _ => 10.0_f32.powf(((i * 73 % 101) as f32 / 10.0) - 6.0),
        });
        let mut actual = [0.0; H];
        extract_cepstral_envelope::<N, H, F>(&magnitudes, &mut actual);
        // Preserve the established endpoint convention, then evaluate the
        // inverse and forward transforms directly in f64, without an FFT.
        let mut logs = vec![0.0_f64; N];
        for i in 0..H {
            logs[i] = f64::from(magnitudes[i].max(1e-6)).ln();
        }
        for i in 1..H - 1 {
            logs[N - i] = logs[i];
        }
        let cepstrum: Vec<_> = (0..64.min(H))
            .map(|k| {
                logs.iter()
                    .enumerate()
                    .map(|(i, x)| x * (std::f64::consts::TAU * (i * k) as f64 / N as f64).cos())
                    .sum::<f64>()
                    / N as f64
            })
            .collect();
        for (i, &value) in actual.iter().enumerate() {
            let log = cepstrum[0]
                + 2.0
                    * cepstrum
                        .iter()
                        .enumerate()
                        .skip(1)
                        .map(|(k, c)| c * (std::f64::consts::TAU * (i * k) as f64 / N as f64).cos())
                        .sum::<f64>();
            let expected = log.exp();
            let error = (f64::from(value) / expected - 1.0).abs();
            assert!(
                value.is_finite() && error < 0.000025,
                "N={N} shape={shape} bin={i}: {value} vs {expected}, relative error {error}"
            );
        }
    }
}

#[test]
fn real_cepstrum_matches_independent_double_precision_dft() {
    check::<512, 256, Fft512>();
    check::<1024, 512, Fft1024>();
    check::<2048, 1024, Fft2048>();
    check::<4096, 2048, Fft4096>();
}

static FORWARDS: AtomicUsize = AtomicUsize::new(0);
struct Budget;
impl FftOps<1024, 512> for Budget {
    fn forward_fft(input: &mut [f32; 1024]) -> &mut [microfft::Complex32] {
        FORWARDS.fetch_add(1, Ordering::Relaxed);
        Fft1024::forward_fft(input)
    }
    fn inverse_fft(_: &mut [microfft::Complex32; 1024]) -> &mut [microfft::Complex32; 1024] {
        panic!("formant envelope must not require a full complex inverse FFT")
    }
    fn get_hann_window() -> &'static [f32; 1024] {
        Fft1024::get_hann_window()
    }
}
#[test]
fn envelope_uses_only_two_real_transforms() {
    let mut envelope = [0.0; 512];
    extract_cepstral_envelope::<1024, 512, Budget>(&[0.1; 512], &mut envelope);
    assert_eq!(FORWARDS.load(Ordering::Relaxed), 2);
}

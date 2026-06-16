use core::f32::consts::PI;

use libm::{atan2f, cosf, floorf, fmodf, roundf, sinf, sqrtf};

use crate::audio::find_nearest_note_frequency;

#[inline(always)]
pub fn calculate_updates<const N: usize>(
    index: usize,
    analysis_frequencies: &[f32],
    analysis_magnitudes: &[f32],
    transition_speed: f32,
) -> Option<(usize, f32, f32)> {
    if index >= analysis_frequencies.len() || index >= analysis_magnitudes.len() {
        return None;
    }
    let exact_frequency = analysis_frequencies[index];
    let target_frequency = find_nearest_note_frequency(exact_frequency);
    let pitch_shift = target_frequency / exact_frequency;

    let new_bin = floorf(index as f32 * pitch_shift + 0.5) as usize;

    if new_bin < N / 2 {
        let updated_magnitude = transition_speed * analysis_magnitudes[new_bin]
            + (1.0 - transition_speed) * analysis_magnitudes[index];
        let updated_frequency = exact_frequency * pitch_shift;
        Some((new_bin, updated_magnitude, updated_frequency))
    } else {
        None
    }
}

#[inline(always)]
pub fn find_fundamental_frequency(
    analysis_magnitudes: &[f32],
    harmonic_product_spectrum: &mut [f32],
    config: &crate::VocalEffectsConfig,
) -> usize {
    let bin_width = config.sample_rate / config.fft_size as f32;
    let max_bin = (config.max_frequency / bin_width) as usize;
    let min_bin = (config.min_frequency / bin_width).max(1.0) as usize;

    // Ensure we don't read past the input array
    let safe_max_bin = max_bin.min(analysis_magnitudes.len());

    // Find the maximum magnitude for normalization
    let mut max_mag = 0.0f32;
    for &mag in &analysis_magnitudes[min_bin..safe_max_bin] {
        if mag > max_mag {
            max_mag = mag;
        }
    }

    // Exit early for silence
    if max_mag < 1e-6 {
        return min_bin;
    }

    let noise_threshold = max_mag * config.hps_noise_floor_ratio;
    // Reciprocal of max_mag so normalization is a multiply instead of repeated divides
    let inv_max_mag = 1.0 / max_mag;

    // Filtering out any data below the min hz and normalizing
    for (j, hps) in harmonic_product_spectrum[min_bin..safe_max_bin].iter_mut().enumerate() {
        let mag = analysis_magnitudes[min_bin + j];
        *hps = if mag > noise_threshold {
            mag * inv_max_mag
        } else {
            0.0
        };
    }

    // Multiply with downsampled versions
    for &factor in config.hps_downsample_factors[..config.hps_num_factors].iter() {
        // i*factor must be < safe_max_bin
        let limit = safe_max_bin / factor;

        for (j, hps) in harmonic_product_spectrum[min_bin..limit].iter_mut().enumerate() {
            let harmonic_idx = (min_bin + j) * factor;
            *hps *= analysis_magnitudes[harmonic_idx] * inv_max_mag;
        }
    }

    // Find the bin with the maximum HPS value
    let mut max_val = 0.0;
    let mut best_bin = min_bin;

    for (j, &hps_val) in harmonic_product_spectrum[min_bin..safe_max_bin].iter().enumerate() {
        if hps_val > max_val {
            max_val = hps_val;
            best_bin = min_bin + j;
        }
    }

    // Sub-octave disambiguation: HPS is biased toward harmonics — on bright or
    // breathy voices it frequently peaks at 2× or 3× the true fundamental because
    // those harmonics are stronger than the fundamental itself.  If the sub-octave
    // (or sub-third) of the detected bin carries clear spectral energy, the actual
    // fundamental is likely down there.  One correction step handles the most common
    // cases without risking runaway descent on legitimate mid-range fundamentals.
    for &divisor in &[2usize, 3usize] {
        let lower_bin = best_bin / divisor;
        if lower_bin >= min_bin && analysis_magnitudes[lower_bin] > noise_threshold {
            best_bin = lower_bin;
            break;
        }
    }

    best_bin
}

/// YIN-based monophonic pitch detection in the time domain.
///
/// Returns the estimated fundamental frequency in Hz, or 0.0 when the signal
/// is silent or insufficiently periodic.
///
/// Internally decimates the input by 8× (yielding ~6 kHz effective sample rate)
/// so the inner-loop work is ~6 K multiply-adds (~0.06 ms on M7 @ 480 MHz).
/// Vocal coverage: 80–600 Hz (full male range, most of female range).
///
/// The algorithm follows the YIN paper (de Cheveigné & Kawahara, 2002):
///   1. Compute the difference function d[τ] = Σ (x[j] − x[j+τ])²
///   2. Normalise into the Cumulative Mean Normalised Difference (CMNDF):
///      d′[τ] = d[τ] · τ / Σ_{j=1}^{τ} d[j]
///   3. Find the first τ where d′[τ] < THRESHOLD (0.15) and return fs/τ.
///   4. If no crossing is found, return the τ with the global minimum (or 0.0
///      if that minimum is too high, indicating no detectable pitch).
///   5. Refine with parabolic interpolation for sub-sample lag accuracy.
#[inline(always)]
pub fn find_pitch_yin(x: &[f32], config: &crate::VocalEffectsConfig) -> f32 {
    // 8× decimation: effective rate ≈ 6 kHz, 128-sample buffer.
    // Reduces inner-loop work 4× vs. the 4× variant while still covering
    // the full singing range 80–600 Hz.
    const DECIMATION: usize = 8;
    const DEC_BUF: usize = 128; // 1024 / 8

    // Intentionally narrower than config.min/max_frequency (those drive HPS).
    // max_lag = 6000/80 ≈ 75 ≤ DEC_BUF/2, ensuring a full period of signal
    // is always available for the inner window at the longest lag.
    const MIN_VOCAL_HZ: f32 = 80.0;
    const MAX_VOCAL_HZ: f32 = 600.0;

    // Aperiodicity threshold: d′[τ] < THRESHOLD → accept as pitch.
    // 0.15 is the value recommended in the YIN paper for noisy conditions.
    const THRESHOLD: f32 = 0.15;

    let effective_sr = config.sample_rate / DECIMATION as f32;
    let n_dec = (x.len() / DECIMATION).min(DEC_BUF);

    let min_lag = (effective_sr / MAX_VOCAL_HZ).max(1.0) as usize;
    let max_lag = ((effective_sr / MIN_VOCAL_HZ) as usize).min(n_dec.saturating_sub(2));

    if min_lag >= max_lag {
        return 0.0;
    }

    // Build decimated signal (4× decimation, no explicit anti-alias filter;
    // vocal energy above the 6 kHz alias boundary is negligible in practice).
    let mut xd = [0.0f32; DEC_BUF];
    for i in 0..n_dec {
        xd[i] = x[i * DECIMATION];
    }

    // Silence gate
    let mut energy = 0.0f32;
    for &s in &xd[..n_dec] {
        energy += s * s;
    }
    if energy < 1e-8 {
        return 0.0;
    }

    // ── Step 2: CMNDF ──────────────────────────────────────────────────────────
    // Pre-fill running_sum with d[1..min_lag) so the normalisation is correct
    // when we enter the search range at tau = min_lag.
    let mut running_sum = 0.0f32;
    for tau in 1..min_lag {
        let window = n_dec - tau;
        let mut d = 0.0f32;
        for j in 0..window {
            let diff = xd[j] - xd[j + tau];
            d += diff * diff;
        }
        running_sum += d;
    }

    let mut best_tau = min_lag;
    let mut best_d_prime = 2.0f32; // above any normalised value

    for tau in min_lag..=max_lag {
        let window = n_dec - tau;
        let mut d_tau = 0.0f32;
        for j in 0..window {
            let diff = xd[j] - xd[j + tau];
            d_tau += diff * diff;
        }
        running_sum += d_tau;

        // d′[τ] = d[τ] · τ / Σ_{j=1}^{τ} d[j]
        let d_prime = if running_sum > 1e-10 {
            d_tau * tau as f32 / running_sum
        } else {
            1.0
        };

        if d_prime < THRESHOLD {
            // ── Step 3: first below-threshold crossing ─────────────────────
            best_tau = tau;
            best_d_prime = d_prime;
            break;
        }

        // Track global minimum as fallback (Step 4)
        if d_prime < best_d_prime {
            best_d_prime = d_prime;
            best_tau = tau;
        }
    }

    // Step 4 fallback rejection: if even the global minimum is aperiodic, give up.
    if best_d_prime > 0.5 {
        return 0.0;
    }

    // ── Step 5: parabolic interpolation ────────────────────────────────────────
    // Refine the integer best_tau by fitting a parabola through d[τ-1], d[τ], d[τ+1].
    // Minimum of f(x)=ax²+bx+c at x = -b/2a = (d0-d2)/(2*(2*d1-d0-d2)).
    let tau_f = if best_tau > min_lag && best_tau < max_lag {
        let t0 = best_tau - 1;
        let t1 = best_tau;
        let t2 = best_tau + 1;

        let mut d0 = 0.0f32;
        for j in 0..(n_dec - t0) {
            let diff = xd[j] - xd[j + t0];
            d0 += diff * diff;
        }
        let mut d1 = 0.0f32;
        for j in 0..(n_dec - t1) {
            let diff = xd[j] - xd[j + t1];
            d1 += diff * diff;
        }
        let mut d2 = 0.0f32;
        for j in 0..(n_dec - t2) {
            let diff = xd[j] - xd[j + t2];
            d2 += diff * diff;
        }

        // denom = 2·d1 - d0 - d2 = −2a; correction = (d0−d2)/(−2·denom) = −(d0−d2)/(2·denom)
        let denom = 2.0 * d1 - d0 - d2;
        if denom.abs() > 1e-10 {
            (best_tau as f32 - (d0 - d2) / (2.0 * denom)).max(1.0)
        } else {
            best_tau as f32
        }
    } else {
        best_tau as f32
    };

    effective_sr / tau_f
}

#[inline(always)]
pub fn collect_harmonics(fundamental_index: usize) -> [usize; 8] {
    let mut harmonics = [0; 8];
    for n in 1..=8 {
        let harmonic_index = fundamental_index * n;
        harmonics[n - 1] = harmonic_index;
    }
    harmonics
}

#[inline(always)]
pub fn sample_rate_reduce(
    sample: f32,
    factor: i8,
    hold_counter: &mut i32,
    held_value: &mut f32,
) -> f32 {
    // If we're at the start of the "hold" cycle, update the held sample
    if *hold_counter == 0 {
        *held_value = sample;
    }
    // Increment the hold_counter (wrapping around "factor")
    //TODO: this can cause a panic if devide by 0

    if factor != 0 {
        *hold_counter = (*hold_counter + 1) % factor as i32;
    }

    // Always return the held_value (which may have just been updated)
    *held_value
}

#[inline(always)]
pub fn bitcrush(sample: f32, bit_depth: i8) -> f32 {
    let levels = (1u64 << bit_depth) as f32;
    // Normalize sample from [-1,1] to [0,1]
    let normalized = (sample + 1.0) / 2.0;
    // Quantize the sample using libm's roundf
    let quantized = roundf(normalized * levels) / levels;
    // Map back to [-1,1]
    quantized * 2.0 - 1.0
}

#[inline(always)]
pub fn normalize_sample(sample: f32, target_peak: f32) -> f32 {
    let abs_sample = sample.abs();
    if abs_sample > target_peak {
        // Scale the sample down to target_peak while preserving its sign.
        sample * (target_peak / abs_sample)
    } else {
        sample
    }
}

#[inline(always)]
pub fn wrap_phase(phase_in: f32) -> f32 {
    if phase_in >= 0.0 {
        return fmodf(phase_in + PI, 2.0 * PI) - PI;
    }
    fmodf(phase_in - PI, -2.0 * PI) + PI
}

// Phase vocoder analysis function
#[inline(always)]
pub fn perform_phase_vocoder_analysis<const N: usize, const HALF_N: usize>(
    fft: &[microfft::Complex32],
    last_input_phases: &mut [f32; N],
    analysis_magnitudes: &mut [f32; HALF_N],
    analysis_frequencies: &mut [f32; HALF_N],
    hop_size: usize,
) {
    for i in 0..fft.len() {
        let amplitude = sqrtf(fft[i].re * fft[i].re + fft[i].im * fft[i].im);
        let phase = atan2f(fft[i].im, fft[i].re);

        // Phase difference for exact frequency
        let mut phase_diff = phase - last_input_phases[i];

        let bin_centre_frequency = 2.0 * PI * i as f32 / N as f32;
        phase_diff = wrap_phase(phase_diff - bin_centre_frequency * hop_size as f32);
        let bin_deviation = phase_diff * N as f32 / hop_size as f32 / (2.0 * PI);

        analysis_frequencies[i] = i as f32 + bin_deviation;
        analysis_magnitudes[i] = amplitude;

        last_input_phases[i] = phase;
    }
}

// Phase vocoder synthesis function
#[inline(always)]
pub fn perform_phase_vocoder_synthesis<const N: usize>(
    synthesis_magnitudes: &mut [f32; N],
    synthesis_frequencies: &mut [f32; N],
    last_output_phases: &mut [f32; N],
    full_spectrum: &mut [microfft::Complex32; N],
    hop_size: usize,
) {
    for i in 0..N / 2 {
        let amplitude = synthesis_magnitudes[i];
        let bin_deviation = synthesis_frequencies[i] - i as f32;

        let mut phase_diff = bin_deviation * 2.0 * PI * hop_size as f32 / N as f32;
        let bin_centre_frequency = 2.0 * PI * i as f32 / N as f32;
        phase_diff += bin_centre_frequency * hop_size as f32;

        let out_phase = wrap_phase(last_output_phases[i] + phase_diff);
        last_output_phases[i] = out_phase;

        full_spectrum[i] = microfft::Complex32 {
            re: amplitude * cosf(out_phase),
            im: amplitude * sinf(out_phase),
        };

        if i > 0 && i < (N / 2) {
            full_spectrum[N - i] = full_spectrum[i].conj();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_nearest_note_frequency_exact_match() {
        let frequency = 440.0;
        let expected = 440.0;
        let result = find_nearest_note_frequency(frequency);
        assert_eq!(result, expected);
    }

    #[test]
    fn test_find_nearest_note_frequency_in_between() {
        let frequency = 445.0;
        let expected = 440.0;
        let result = find_nearest_note_frequency(frequency);
        assert_eq!(result, expected);
    }

    #[test]
    fn test_find_nearest_note_frequency_below_range() {
        let frequency = 10.0;
        let expected = 16.35;
        let result = find_nearest_note_frequency(frequency);
        assert_eq!(result, expected);
    }

    #[test]
    fn test_find_nearest_note_frequency_above_range() {
        let frequency = 5000.0;
        let result = find_nearest_note_frequency(frequency);
        // Should be close to the highest frequency in our generated scales
        assert!(result > 4900.0 && result < 5100.0);
    }

    #[test]
    fn test_find_nearest_note_frequency_mid_point() {
        let frequency = 55.0;
        let result = find_nearest_note_frequency(frequency);
        // Should be close to 55.0 Hz (A1)
        assert!((result - 55.0).abs() < 0.1);
    }

    #[test]
    fn test_find_nearest_note_frequency_edge_case_low() {
        let frequency = 16.0;
        let result = find_nearest_note_frequency(frequency);
        // Should be close to C0 (16.35 Hz)
        assert!((result - 16.35).abs() < 0.1);
    }

    #[test]
    fn test_find_nearest_note_frequency_edge_case_high() {
        let frequency = 4999.0;
        let result = find_nearest_note_frequency(frequency);
        // Should be close to the highest frequency in our generated scales
        assert!(result > 4900.0 && result < 5100.0);
    }

    #[test]
    fn test_find_nearest_note_frequency_very_close_lower() {
        let frequency = 110.1;
        let expected = 110.0;
        let result = find_nearest_note_frequency(frequency);
        assert_eq!(result, expected);
    }

    #[test]
    fn test_find_nearest_note_frequency_very_close_upper() {
        let frequency = 109.9;
        let expected = 110.0;
        let result = find_nearest_note_frequency(frequency);
        assert_eq!(result, expected);
    }
}

#[cfg(test)]
mod detect_fun_freq_tests {
    use super::*;

    fn test_config() -> crate::VocalEffectsConfig {
        crate::VocalEffectsConfig {
            min_frequency: 80.0,
            max_frequency: 500.0,
            sample_rate: 48_014.312,
            fft_size: 1024,
            ..crate::VocalEffectsConfig::default()
        }
    }

    #[test]
    fn test_silence_detection() {
        let config = test_config();
        let analysis_magnitudes = [0.0; 1024];
        let mut harmonic_product_spectrum = [0.0; 1024];

        let result = find_fundamental_frequency(
            &analysis_magnitudes,
            &mut harmonic_product_spectrum,
            &config,
        );

        let bin_width = config.sample_rate / config.fft_size as f32;
        let min_bin = (config.min_frequency / bin_width).max(1.0) as usize;
        assert_eq!(result, min_bin, "Silence should return min_bin");
    }

    #[test]
    fn test_single_frequency_peak() {
        let config = test_config();
        let mut analysis_magnitudes = [0.1; 1024];
        let mut harmonic_product_spectrum = [0.0; 1024];

        const PEAK_BIN: usize = 5;
        analysis_magnitudes[PEAK_BIN] = 1.0;
        analysis_magnitudes[PEAK_BIN * 2] = 0.5;
        analysis_magnitudes[PEAK_BIN * 3] = 0.3;

        let result = find_fundamental_frequency(
            &analysis_magnitudes,
            &mut harmonic_product_spectrum,
            &config,
        );

        assert!(
            (PEAK_BIN - 2..=PEAK_BIN + 2).contains(&result),
            "Should detect fundamental near bin {}, got {}",
            PEAK_BIN,
            result
        );
    }

    #[test]
    fn test_vocal_frequency_range() {
        let config = test_config();
        let mut analysis_magnitudes = [0.0; 1024];
        let mut harmonic_product_spectrum = [0.0; 1024];

        const VOCAL_BIN: usize = 3;
        analysis_magnitudes[VOCAL_BIN] = 1.0;
        analysis_magnitudes[VOCAL_BIN * 2] = 0.8;
        analysis_magnitudes[VOCAL_BIN * 3] = 0.6;
        analysis_magnitudes[VOCAL_BIN * 4] = 0.4;

        let result = find_fundamental_frequency(
            &analysis_magnitudes,
            &mut harmonic_product_spectrum,
            &config,
        );

        let bin_width = config.sample_rate / config.fft_size as f32;
        let min_bin = (config.min_frequency / bin_width).max(1.0) as usize;
        let max_bin = (config.max_frequency / bin_width) as usize;
        assert!(
            result >= min_bin && result <= max_bin,
            "Result {} should be in vocal range [{}, {}]",
            result,
            min_bin,
            max_bin
        );
    }

    #[test]
    fn test_noise_floor_filtering() {
        let config = test_config();
        let mut analysis_magnitudes = [0.0; 1024];
        let mut harmonic_product_spectrum = [0.0; 1024];

        analysis_magnitudes.fill(0.05);

        const SIGNAL_BIN: usize = 4;
        analysis_magnitudes[SIGNAL_BIN] = 1.0;
        analysis_magnitudes[SIGNAL_BIN * 2] = 0.7;
        analysis_magnitudes[SIGNAL_BIN * 3] = 0.5;

        let result = find_fundamental_frequency(
            &analysis_magnitudes,
            &mut harmonic_product_spectrum,
            &config,
        );

        assert!(
            (SIGNAL_BIN - 2..=SIGNAL_BIN + 2).contains(&result),
            "Should detect signal at bin {}, got {}",
            SIGNAL_BIN,
            result
        );
    }

    #[test]
    fn test_harmonic_product_spectrum_calculation() {
        let config = test_config();
        let mut analysis_magnitudes = [0.0; 1024];
        let mut harmonic_product_spectrum = [0.0; 1024];

        const FUNDAMENTAL: usize = 6;
        analysis_magnitudes[FUNDAMENTAL] = 1.0;
        analysis_magnitudes[FUNDAMENTAL * 2] = 0.8;
        analysis_magnitudes[FUNDAMENTAL * 3] = 0.6;
        analysis_magnitudes[FUNDAMENTAL * 4] = 0.4;

        let result = find_fundamental_frequency(
            &analysis_magnitudes,
            &mut harmonic_product_spectrum,
            &config,
        );

        assert!(
            (FUNDAMENTAL - 1..=FUNDAMENTAL + 1).contains(&result),
            "HPS should identify fundamental at bin {}, got {}",
            FUNDAMENTAL,
            result
        );
        assert!(harmonic_product_spectrum[result] > 0.0, "HPS at result should be non-zero");
    }

    #[test]
    fn test_upper_frequency_limit() {
        let config = test_config();
        let mut analysis_magnitudes = [0.0; 1024];
        let mut harmonic_product_spectrum = [0.0; 1024];

        const HIGH_BIN: usize = 20;
        analysis_magnitudes[HIGH_BIN] = 1.0;

        let result = find_fundamental_frequency(
            &analysis_magnitudes,
            &mut harmonic_product_spectrum,
            &config,
        );

        let bin_width = config.sample_rate / config.fft_size as f32;
        let max_bin = (config.max_frequency / bin_width) as usize;
        assert!(result <= max_bin, "Result {} should not exceed max_bin {}", result, max_bin);
    }

    #[test]
    fn test_buffer_boundary_safety() {
        let config = test_config();
        let analysis_magnitudes = [0.5; 1024];
        let mut harmonic_product_spectrum = [0.0; 1024];

        let result = find_fundamental_frequency(
            &analysis_magnitudes,
            &mut harmonic_product_spectrum,
            &config,
        );

        assert!(result < 1024, "Result should be within buffer bounds");
    }

    #[test]
    fn test_normalization() {
        let config = test_config();
        let mut analysis_magnitudes = [0.0; 1024];
        let mut harmonic_product_spectrum = [0.0; 1024];

        const WEAK_BIN: usize = 3;
        const STRONG_BIN: usize = 5;

        analysis_magnitudes[WEAK_BIN] = 0.1;
        analysis_magnitudes[WEAK_BIN * 2] = 0.08;
        analysis_magnitudes[WEAK_BIN * 3] = 0.06;

        analysis_magnitudes[STRONG_BIN] = 10.0;
        analysis_magnitudes[STRONG_BIN * 2] = 8.0;
        analysis_magnitudes[STRONG_BIN * 3] = 6.0;

        let result = find_fundamental_frequency(
            &analysis_magnitudes,
            &mut harmonic_product_spectrum,
            &config,
        );

        assert!(
            (STRONG_BIN - 2..=STRONG_BIN + 2).contains(&result),
            "Should detect strong harmonic at bin {}, got {}",
            STRONG_BIN,
            result
        );
    }

    // Verify sub-octave disambiguation: when HPS peaks at 2× the true fundamental
    // (because the 2nd harmonic is dominant — common on bright/breathy voices), the
    // detected bin should be corrected back to the fundamental.
    #[test]
    fn test_octave_disambiguation_2x() {
        let config = test_config();
        let mut analysis_magnitudes = [0.0; 1024];
        let mut harmonic_product_spectrum = [0.0; 1024];

        // True fundamental at bin 4 (~188 Hz), but 2nd harmonic is 3× stronger,
        // which would normally fool HPS into picking bin 8.
        const TRUE_FUNDAMENTAL: usize = 4;
        analysis_magnitudes[TRUE_FUNDAMENTAL] = 0.5;       // fundamental — present but weaker
        analysis_magnitudes[TRUE_FUNDAMENTAL * 2] = 1.0;   // 2nd harmonic — dominant
        analysis_magnitudes[TRUE_FUNDAMENTAL * 3] = 0.7;
        analysis_magnitudes[TRUE_FUNDAMENTAL * 4] = 0.4;

        let result = find_fundamental_frequency(
            &analysis_magnitudes,
            &mut harmonic_product_spectrum,
            &config,
        );

        assert_eq!(
            result, TRUE_FUNDAMENTAL,
            "Sub-octave disambiguation should recover fundamental bin {}, got {}",
            TRUE_FUNDAMENTAL, result
        );
    }
}

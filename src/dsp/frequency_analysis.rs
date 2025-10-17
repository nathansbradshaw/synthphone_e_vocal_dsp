use core::f32::consts::PI;

use libm::{atan2f, cosf, fabsf, floorf, fmodf, roundf, sinf, sqrtf};

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
pub fn find_fundamental_frequency(analysis_magnitudes: &[f32], harmonic_product_spectrum: &mut [f32], ) -> usize {
    const MIN_HZ: f32 = 80.0; // Minimum vocal F0
    const MAX_HZ: f32 = 500.0; // Maximum typical vocal F0

    //TODO: pass in these constants----------
    const FFT_SIZE: usize = 1024;
    const SAMPLE_RATE: f32 = 48_014.312;
    const NOISE_FLOOR_RATIO: f32 = 0.1;
    //---------------------------------------

    const BIN_WIDTH: f32 = SAMPLE_RATE / FFT_SIZE as f32;
    const MAX_BIN: usize = (MAX_HZ as f32 / BIN_WIDTH) as usize;
    const MIN_BIN: usize = (MIN_HZ / BIN_WIDTH).max(1.0) as usize;

    const DOWNSAMPLE_FACTORS: [usize; 3] = [2, 3, 4,];


    // Ensure we don't read past the input array
    let safe_max_bin = MAX_BIN.min(analysis_magnitudes.len());

    // Find the maximum magnitude for normalization
    let mut max_mag = 0.0f32;
    for i in MIN_BIN..safe_max_bin {
        if analysis_magnitudes[i] > max_mag {
            max_mag = analysis_magnitudes[i];
        }
    }

    // Exit early for silence
    if max_mag < 1e-6 {
        return MIN_BIN;
    }

    let noise_threshold = max_mag * NOISE_FLOOR_RATIO;
    let max_mag_recip = 1.0 / max_mag;

    // Filtering out any data below the min hz and normalizing
    for i in MIN_BIN..safe_max_bin {
        // Apply noise gate
        if analysis_magnitudes[i] > noise_threshold {
            // Copy spectrum within range and normalize to [0.0, 1.0]
            harmonic_product_spectrum[i] = analysis_magnitudes[i] / max_mag_recip ;
        } else {
            harmonic_product_spectrum[i] = 0.0;
        }
    }

    // Multiply with downsampled versions
    for &factor in DOWNSAMPLE_FACTORS.iter() {
        // i*factor must be < safe_max_bin
        let limit = safe_max_bin / factor;

        for i in MIN_BIN..limit {
            let harmonic_idx = i * factor;

            harmonic_product_spectrum[i] *= analysis_magnitudes[harmonic_idx] / max_mag_recip;
        }
    }

    // Find the bin with the maximum HPS value
    let mut max_val = 0.0;
    let mut max_bin = MIN_BIN;

    for i in MIN_BIN..safe_max_bin {
        if harmonic_product_spectrum[i] > max_val {
            max_val = harmonic_product_spectrum[i];
            max_bin = i;
        }
    }

    // Convert bin index to frequency
    //let bin_width = SAMPLE_RATE as usize / FFT_SIZE as usize;
    max_bin
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
    let abs_sample = fabsf(sample);
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

    #[test]
    fn test_silence_detection() {
        let analysis_magnitudes = [0.0; 1024];
        let mut harmonic_product_spectrum = [0.0; 1024];
        
        let result = find_fundamental_frequency(&analysis_magnitudes, &mut harmonic_product_spectrum);
        
        // For silence, should return MIN_BIN (around 80 Hz)
        const MIN_HZ: f32 = 80.0;
        const SAMPLE_RATE: f32 = 48_014.312;
        const FFT_SIZE: usize = 1024;
        const BIN_WIDTH: f32 = SAMPLE_RATE / FFT_SIZE as f32;
        const MIN_BIN: usize = (MIN_HZ / BIN_WIDTH).max(1.0) as usize;
        
        assert_eq!(result, MIN_BIN, "Silence should return MIN_BIN");
    }

    #[test]
    fn test_single_frequency_peak() {
        let mut analysis_magnitudes = [0.1; 1024];
        let mut harmonic_product_spectrum = [0.0; 1024];
        
        // Create a strong peak at bin 100 (around 100 * 46.9 Hz ≈ 4690 Hz)
        // This is outside vocal range, so let's use bin 5 instead (around 234 Hz)
        const PEAK_BIN: usize = 5;
        analysis_magnitudes[PEAK_BIN] = 1.0;
        analysis_magnitudes[PEAK_BIN * 2] = 0.5; // Add 2nd harmonic
        analysis_magnitudes[PEAK_BIN * 3] = 0.3; // Add 3rd harmonic
        
        let result = find_fundamental_frequency(&analysis_magnitudes, &mut harmonic_product_spectrum);
        
        // Should detect the fundamental at PEAK_BIN or nearby
        assert!(
            result >= PEAK_BIN - 2 && result <= PEAK_BIN + 2,
            "Should detect fundamental near bin {}, got {}",
            PEAK_BIN,
            result
        );
    }

    #[test]
    fn test_vocal_frequency_range() {
        let mut analysis_magnitudes = [0.0; 1024];
        let mut harmonic_product_spectrum = [0.0; 1024];
        
        // Male voice typical F0: ~120 Hz
        // 120 Hz / 46.9 Hz/bin ≈ bin 2.56, so use bin 3
        const VOCAL_BIN: usize = 3;
        
        analysis_magnitudes[VOCAL_BIN] = 1.0;
        analysis_magnitudes[VOCAL_BIN * 2] = 0.8;
        analysis_magnitudes[VOCAL_BIN * 3] = 0.6;
        analysis_magnitudes[VOCAL_BIN * 4] = 0.4;
        
        let result = find_fundamental_frequency(&analysis_magnitudes, &mut harmonic_product_spectrum);
        
        // Should detect fundamental in vocal range
        const MIN_HZ: f32 = 80.0;
        const MAX_HZ: f32 = 500.0;
        const SAMPLE_RATE: f32 = 48_014.312;
        const FFT_SIZE: usize = 1024;
        const BIN_WIDTH: f32 = SAMPLE_RATE / FFT_SIZE as f32;
        const MIN_BIN: usize = (MIN_HZ / BIN_WIDTH).max(1.0) as usize;
        const MAX_BIN: usize = (MAX_HZ / BIN_WIDTH) as usize;
        
        assert!(
            result >= MIN_BIN && result <= MAX_BIN,
            "Result {} should be in vocal range [{}, {}]",
            result,
            MIN_BIN,
            MAX_BIN
        );
    }

    #[test]
    fn test_noise_floor_filtering() {
        let mut analysis_magnitudes = [0.0; 1024];
        let mut harmonic_product_spectrum = [0.0; 1024];
        
        // Add low-level noise throughout
        for i in 0..1024 {
            analysis_magnitudes[i] = 0.05;
        }
        
        // Add a strong signal with harmonics
        const SIGNAL_BIN: usize = 4;
        analysis_magnitudes[SIGNAL_BIN] = 1.0;
        analysis_magnitudes[SIGNAL_BIN * 2] = 0.7;
        analysis_magnitudes[SIGNAL_BIN * 3] = 0.5;
        
        let result = find_fundamental_frequency(&analysis_magnitudes, &mut harmonic_product_spectrum);
        
        // Should detect the strong signal, not the noise
        assert!(
            result >= SIGNAL_BIN - 2 && result <= SIGNAL_BIN + 2,
            "Should detect signal at bin {}, got {}",
            SIGNAL_BIN,
            result
        );
    }

    #[test]
    fn test_harmonic_product_spectrum_calculation() {
        let mut analysis_magnitudes = [0.0; 1024];
        let mut harmonic_product_spectrum = [0.0; 1024];
        
        // Create a harmonic series at bin 6
        const FUNDAMENTAL: usize = 6;
        analysis_magnitudes[FUNDAMENTAL] = 1.0;
        analysis_magnitudes[FUNDAMENTAL * 2] = 0.8;
        analysis_magnitudes[FUNDAMENTAL * 3] = 0.6;
        analysis_magnitudes[FUNDAMENTAL * 4] = 0.4;
        
        let result = find_fundamental_frequency(&analysis_magnitudes, &mut harmonic_product_spectrum);
        
        // HPS should strongly identify the fundamental
        assert!(
            result >= FUNDAMENTAL - 1 && result <= FUNDAMENTAL + 1,
            "HPS should identify fundamental at bin {}, got {}",
            FUNDAMENTAL,
            result
        );
        
        // The HPS value at the fundamental should be non-zero
        assert!(
            harmonic_product_spectrum[result] > 0.0,
            "HPS at result should be non-zero"
        );
    }

    #[test]
    fn test_upper_frequency_limit() {
        let mut analysis_magnitudes = [0.0; 1024];
        let mut harmonic_product_spectrum = [0.0; 1024];
        
        // Try to create a peak well above MAX_HZ (500 Hz)
        // 500 Hz / 46.9 Hz/bin ≈ bin 10.7
        const HIGH_BIN: usize = 20; // Well above vocal range
        analysis_magnitudes[HIGH_BIN] = 1.0;
        
        let result = find_fundamental_frequency(&analysis_magnitudes, &mut harmonic_product_spectrum);
        
        // Result should be clamped to MAX_BIN
        const MAX_HZ: f32 = 500.0;
        const SAMPLE_RATE: f32 = 48_014.312;
        const FFT_SIZE: usize = 1024;
        const BIN_WIDTH: f32 = SAMPLE_RATE / FFT_SIZE as f32;
        const MAX_BIN: usize = (MAX_HZ / BIN_WIDTH) as usize;
        
        assert!(
            result <= MAX_BIN,
            "Result {} should not exceed MAX_BIN {}",
            result,
            MAX_BIN
        );
    }

    #[test]
    fn test_buffer_boundary_safety() {
        // Test with a smaller input that's still valid
        let analysis_magnitudes = [0.5; 1024];
        let mut harmonic_product_spectrum = [0.0; 1024];
        
        // Should not panic and should return a valid bin
        let result = find_fundamental_frequency(&analysis_magnitudes, &mut harmonic_product_spectrum);
        
        assert!(result < 1024, "Result should be within buffer bounds");
    }

    #[test]
    fn test_normalization() {
        let mut analysis_magnitudes = [0.0; 1024];
        let mut harmonic_product_spectrum = [0.0; 1024];
        
        // Create peaks with very different magnitudes
        const WEAK_BIN: usize = 3;
        const STRONG_BIN: usize = 5;
        
        analysis_magnitudes[WEAK_BIN] = 0.1;
        analysis_magnitudes[WEAK_BIN * 2] = 0.08;
        analysis_magnitudes[WEAK_BIN * 3] = 0.06;
        
        analysis_magnitudes[STRONG_BIN] = 10.0;
        analysis_magnitudes[STRONG_BIN * 2] = 8.0;
        analysis_magnitudes[STRONG_BIN * 3] = 6.0;
        
        let result = find_fundamental_frequency(&analysis_magnitudes, &mut harmonic_product_spectrum);
        
        // Should detect the fundamental with better harmonic structure (STRONG_BIN)
        // even though it has higher absolute magnitude
        assert!(
            result >= STRONG_BIN - 2 && result <= STRONG_BIN + 2,
            "Should detect strong harmonic at bin {}, got {}",
            STRONG_BIN,
            result
        );
    }
}

use libm::{expf, logf};

use crate::{MusicalSettings, dsp::FftOps};

/// Extract cepstral envelope for formant preservation using generic FFT operations
pub fn extract_cepstral_envelope<const N: usize, const HALF_N: usize, F>(
    analysis_magnitudes: &[f32; HALF_N],
    envelope: &mut [f32; HALF_N],
) where
    F: FftOps<N, HALF_N>,
{
    const LIFTER_CUTOFF: usize = 64;
    let mut log_spectrum = [0.0f32; N];
    let mut cepstrum_buffer = [0.0f32; N];

    // Compute log spectrum with proper symmetry
    for i in 0..HALF_N {
        let mag = analysis_magnitudes[i].max(1e-6_f32);
        let log_mag = logf(mag);
        log_spectrum[i] = log_mag;
    }

    // Mirror the log-magnitude into negative frequency slots for Hermitian symmetry.
    // The log-spectrum is purely real (im=0), so conjugate-symmetric means copying the
    // real part; this makes the IFFT output real-valued (the cepstrum).
    for i in 1..(HALF_N - 1) {
        log_spectrum[N - i] = log_spectrum[i];
    }

    // Only the real cepstrum is used. For a real log-spectrum,
    // Re(IFFT(x))[k] = Re(FFT(x))[k] / N, and it is even in k.
    // A real FFT therefore replaces the full complex inverse transform.
    // Keep the existing endpoint convention above to preserve the effect's
    // sound; this optimization does not change its lifter or refresh cadence.
    let cepstrum = F::forward_fft(&mut log_spectrum);

    // Apply liftering (low-pass in cepstral domain)
    cepstrum_buffer.fill(0.0);
    for i in 0..LIFTER_CUTOFF.min(HALF_N) {
        cepstrum_buffer[i] = cepstrum[i].re / N as f32;
    }
    // Mirror the lifter cutoff for negative quefrencies
    for i in (N - LIFTER_CUTOFF.min(HALF_N) + 1)..N {
        cepstrum_buffer[i] = cepstrum[N - i].re / N as f32;
    }

    // Forward FFT to get smoothed envelope
    let envelope_fft = F::forward_fft(&mut cepstrum_buffer);
    for i in 0..HALF_N {
        envelope[i] = expf(envelope_fft[i].re);
    }
}

pub fn extract_simple_envelope<const HALF_N: usize>(
    analysis_magnitudes: &[f32; HALF_N],
    envelope: &mut [f32; HALF_N],
) {
    const SMOOTH_BINS: usize = 8; // Small window for speed

    for (i, slot) in envelope.iter_mut().enumerate() {
        let start = i.saturating_sub(SMOOTH_BINS);
        let end = (i + SMOOTH_BINS + 1).min(HALF_N);
        let sum: f32 = analysis_magnitudes[start..end].iter().sum();
        *slot = sum / (end - start) as f32;
    }
}

pub fn calculate_pitch_shift(
    _analysis_magnitudes: &[f32],
    _analysis_frequencies: &[f32],
    previous_pitch_shift_ratio: f32,
    settings: &MusicalSettings,
    _bin_width: f32,
    fundamental_frequency: f32,
) -> f32 {
    let mut pitch_shift_ratio = previous_pitch_shift_ratio;

    if fundamental_frequency > 0.001 {
        let raw_ratio = target_ratio(fundamental_frequency, settings);
        const SMOOTHING_FACTOR: f32 = 0.99;
        pitch_shift_ratio =
            raw_ratio * SMOOTHING_FACTOR + previous_pitch_shift_ratio * (1.0 - SMOOTHING_FACTOR);
    }

    pitch_shift_ratio
}

fn target_ratio(fundamental_frequency: f32, settings: &MusicalSettings) -> f32 {
    let target_frequency = if settings.midi_frequencies[0] > 0.0 {
        settings.midi_frequencies[0]
    } else if settings.note == 0 {
        let scale_frequencies = crate::audio::keys::get_scale_by_key(settings.key);
        crate::audio::frequencies::find_nearest_note_in_key(
            fundamental_frequency,
            scale_frequencies,
        )
    } else {
        // Unshifted target: `octave_ratio` is applied once by the caller,
        // uniformly for every target source (MIDI, snapped, fixed note).
        crate::audio::keys::get_frequency(settings.key, settings.note, 1.0, false)
    };
    target_frequency / fundamental_frequency
}

/// Stateful correction. Speed is the fraction of the remaining ratio error
/// corrected per hop; 1 snaps immediately, 0 holds the previous ratio.
/// Invalid/unvoiced input releases correction so a new phrase starts at unity.
pub fn calculate_pitch_shift_with_state(
    ratio: &mut f32,
    settings: &MusicalSettings,
    config: &crate::VocalEffectsConfig,
    fundamental_frequency: f32,
) -> f32 {
    if !fundamental_frequency.is_finite() || fundamental_frequency <= 0.001 {
        *ratio = 1.0;
        return *ratio;
    }
    let target = target_ratio(fundamental_frequency, settings);
    if !target.is_finite() || target <= 0.0 {
        *ratio = 1.0;
        return *ratio;
    }
    let speed = if config.transition_speed.is_finite() {
        config.transition_speed.clamp(0.0, 1.0)
    } else {
        1.0
    };
    let strength = if config.pitch_correction_strength.is_finite() {
        config.pitch_correction_strength.clamp(0.0, 1.0)
    } else {
        1.0
    };
    let previous = if ratio.is_finite() && *ratio > 0.0 {
        *ratio
    } else {
        1.0
    };
    let desired = target * strength + (1.0 - strength);
    *ratio = if speed == 1.0 {
        desired
    } else {
        previous + speed * (desired - previous)
    };
    *ratio
}

use libm::{expf, fabsf, floorf, sqrtf};

use crate::{
    MusicalSettings, VocalEffectsConfig,
    dsp::{
        FftOps, calculate_pitch_shift, extract_cepstral_envelope, extract_simple_envelope,
        frequency_analysis,
    },
};

/// Generic pitch correction processing (pitch correction)
pub fn process_pitch_correction_generic<const N: usize, const HALF_N: usize, F>(
    unwrapped_buffer: &mut [f32; N],
    last_input_phases: &mut [f32; N],
    last_output_phases: &mut [f32; N],
    previous_pitch_shift_ratio: f32,
    config: &VocalEffectsConfig,
    settings: &MusicalSettings,
) -> [f32; N]
where
    F: FftOps<N, HALF_N>,
{
    let hop_size = (N as f32 * config.hop_ratio) as usize;
    let bin_width = config.sample_rate / N as f32;

    let analysis_window_buffer = F::get_hann_window();
    let mut full_spectrum = [microfft::Complex32 { re: 0.0, im: 0.0 }; N];
    let mut analysis_magnitudes = [0.0; HALF_N];
    let mut analysis_frequencies = [0.0; HALF_N];
    let mut synthesis_magnitudes = [0.0; N];
    let mut synthesis_frequencies = [0.0; N];
    let mut envelope = [1.0f32; HALF_N];
    let mut hps_buffer = [1.0f32; HALF_N];

    let formant = settings.formant;

    // Apply windowing
    for i in 0..N {
        unwrapped_buffer[i] *= analysis_window_buffer[i];
    }

    // Forward FFT
    let fft_result = F::forward_fft(unwrapped_buffer);

    // Process frequency bins - limit to the actual number of bins we have arrays for
    frequency_analysis::perform_phase_vocoder_analysis::<N, HALF_N>(
        fft_result,
        last_input_phases,
        &mut analysis_magnitudes,
        &mut analysis_frequencies,
        hop_size,
    );

    let octave_factor = settings.octave as f32 * 0.5;

    // Apply spectral shift
    synthesis_magnitudes.fill(0.0);
    synthesis_frequencies.fill(0.0);

    // Extract formant envelope if needed
    if formant != 0 {
        extract_cepstral_envelope::<N, HALF_N, F>(&analysis_magnitudes, &mut envelope);
    }

    let fundamental_index = frequency_analysis::find_fundamental_frequency(
        &analysis_magnitudes,
        &mut hps_buffer,
        config,
    );
    let fundamental_frequency = analysis_frequencies[fundamental_index] * bin_width;

    // Calculate pitch shift
    let pitch_shift_ratio = calculate_pitch_shift(
        &analysis_magnitudes,
        &analysis_frequencies,
        previous_pitch_shift_ratio,
        settings,
        bin_width,
        fundamental_frequency,
    );

    let formant_ratio = match formant {
        1 => 0.5,
        2 => 2.0,
        _ => 1.0,
    };
    let use_formants = formant != 0;

    for i in 0..HALF_N {
        if analysis_magnitudes[i] <= 1e-8 {
            continue;
        }
        let residual = if use_formants {
            analysis_magnitudes[i] / envelope[i].max(1e-6_f32)
        } else {
            analysis_magnitudes[i]
        };
        let new_bin_f = i as f32 * pitch_shift_ratio;
        let new_bin = (floorf(new_bin_f + 0.5) * octave_factor) as usize;

        if new_bin < HALF_N {
            let shifted_envelope = if use_formants {
                let env_pos = (i as f32 / formant_ratio).clamp(0.0, (HALF_N - 1) as f32);
                let env_idx = env_pos as usize;
                let frac = env_pos - env_idx as f32;
                if env_idx < HALF_N - 1 {
                    envelope[env_idx] * (1.0 - frac) + envelope[env_idx + 1] * frac
                } else {
                    envelope[env_idx]
                }
            } else {
                1.0
            };

            synthesis_magnitudes[new_bin] = residual * shifted_envelope;
            synthesis_frequencies[new_bin] =
                analysis_frequencies[i] * pitch_shift_ratio * octave_factor;
        }
    }

    // Synthesis phase reconstruction
    frequency_analysis::perform_phase_vocoder_synthesis::<N>(
        &mut synthesis_magnitudes,
        &mut synthesis_frequencies,
        last_output_phases,
        &mut full_spectrum,
        hop_size,
    );

    // Inverse FFT
    let time_domain_result = F::inverse_fft(&mut full_spectrum);
    let mut output_samples = [0.0f32; N];

    for i in 0..N {
        let mut sample = time_domain_result[i].re;
        sample *= analysis_window_buffer[i];
        if sample.abs() > 0.95 {
            let sign = if sample >= 0.0 { 1.0 } else { -1.0 };
            let compressed = 0.95 - 0.05 * expf(-fabsf(sample));
            sample = sign * compressed;
        }
        output_samples[i] = sample;
    }

    output_samples
}

/// Generic vocoder processing
pub fn process_vocode_generic<const N: usize, const HALF_N: usize, F>(
    input_buffer: &mut [f32; N],
    carrier_buffer: &mut [f32; N],
    // TODO if we don't need this, remove it
    _last_input_phases: &mut [f32; N],
    _last_output_phases: &mut [f32; N],
    _config: &VocalEffectsConfig,
    _settings: &MusicalSettings,
) -> [f32; N]
where
    F: FftOps<N, HALF_N>,
{
    let analysis_window_buffer = F::get_hann_window();
    let mut full_spectrum = [microfft::Complex32 { re: 0.0, im: 0.0 }; N];

    // Apply windowing to both inputs
    for i in 0..N {
        input_buffer[i] *= analysis_window_buffer[i];
        carrier_buffer[i] *= analysis_window_buffer[i];
    }

    // Forward FFT on both signals
    let modulator_fft = F::forward_fft(input_buffer);
    let carrier_fft = F::forward_fft(carrier_buffer);

    // Process first half of spectrum (including DC and Nyquist)
    let num_bins = HALF_N.min(modulator_fft.len()).min(carrier_fft.len());
    for i in 0..num_bins {
        // Get modulator magnitude (vocal envelope)
        let mod_mag = sqrtf(
            modulator_fft[i].re * modulator_fft[i].re + modulator_fft[i].im * modulator_fft[i].im,
        );

        // Get carrier magnitude
        let car_mag =
            sqrtf(carrier_fft[i].re * carrier_fft[i].re + carrier_fft[i].im * carrier_fft[i].im);

        // Scale carrier by modulator envelope
        let scale_factor = if car_mag > 0.0001 {
            mod_mag / car_mag
        } else {
            0.0
        };

        // Apply scaling to carrier, keeping carrier phase
        full_spectrum[i].re = carrier_fft[i].re * scale_factor;
        full_spectrum[i].im = carrier_fft[i].im * scale_factor;

        // Conjugate symmetry for real output
        if i > 0 && i < num_bins {
            full_spectrum[N - i].re = full_spectrum[i].re;
            full_spectrum[N - i].im = -full_spectrum[i].im;
        }
    }

    // Inverse FFT
    let time_domain_result = F::inverse_fft(&mut full_spectrum);
    let mut output_samples = [0.0f32; N];

    for i in 0..N {
        let mut sample = time_domain_result[i].re;
        sample *= analysis_window_buffer[i];
        output_samples[i] = sample;
    }

    output_samples
}

/// Generic dry processing (pitch shifting with formant preservation but no correction)
pub fn process_dry_generic<const N: usize, const HALF_N: usize, F>(
    unwrapped_buffer: &mut [f32; N],
    synth_buffer: Option<&mut [f32; N]>,
    last_input_phases: &mut [f32; N],
    last_output_phases: &mut [f32; N],
    config: &VocalEffectsConfig,
    settings: &MusicalSettings,
) -> [f32; N]
where
    F: FftOps<N, HALF_N>,
{
    let hop_size = (N as f32 * config.hop_ratio) as usize;
    let analysis_window_buffer = F::get_hann_window();
    let mut full_spectrum = [microfft::Complex32 { re: 0.0, im: 0.0 }; N];
    let mut analysis_magnitudes = [0.0; HALF_N];
    let mut analysis_frequencies = [0.0; HALF_N];
    let mut synthesis_magnitudes = [0.0; N];
    let mut synthesis_frequencies = [0.0; N];
    let mut envelope = [1.0f32; HALF_N];

    let formant = settings.formant;
    let note = settings.note;

    // Apply windowing
    for i in 0..N {
        unwrapped_buffer[i] *= analysis_window_buffer[i];
    }

    // Forward FFT
    let fft_result = F::forward_fft(unwrapped_buffer);

    let octave_factor = settings.octave as f32 * 0.5;
    let pitch_shift_ratio = octave_factor;

    // If no effects, just pass through
    if formant == 0 && (pitch_shift_ratio > 0.99 && pitch_shift_ratio < 1.01) {
        // Direct pass-through - just copy spectrum
        full_spectrum[..HALF_N].copy_from_slice(&fft_result[..HALF_N]);
        for i in 1..HALF_N {
            if N - i < full_spectrum.len() {
                full_spectrum[N - i] = fft_result[i].conj();
            }
        }
    } else {
        // Analysis phase
        frequency_analysis::perform_phase_vocoder_analysis::<N, HALF_N>(
            fft_result,
            last_input_phases,
            &mut analysis_magnitudes,
            &mut analysis_frequencies,
            hop_size,
        );

        // Extract formant envelope if needed
        if formant != 0 {
            extract_cepstral_envelope::<N, HALF_N, F>(&analysis_magnitudes, &mut envelope);
        }

        // Zero synthesis arrays
        synthesis_magnitudes.fill(0.0);
        synthesis_frequencies.fill(0.0);

        let formant_ratio = match formant {
            1 => 0.8, // Lower formants
            2 => 1.3, // Raise formants
            _ => 1.0, // No formant shift
        };

        // Pitch and formant shifting
        for i in 0..HALF_N {
            let residual = if formant != 0 {
                analysis_magnitudes[i] / envelope[i].max(1e-6)
            } else {
                analysis_magnitudes[i]
            };

            let new_bin = (floorf(i as f32 * pitch_shift_ratio + 0.5) * octave_factor) as usize;

            if new_bin < HALF_N {
                let shifted_envelope = if formant != 0 {
                    let env_pos = (i as f32 / formant_ratio).clamp(0.0, (HALF_N - 1) as f32);
                    let env_idx = env_pos as usize;
                    let frac = env_pos - env_idx as f32;

                    if env_idx < HALF_N - 1 {
                        envelope[env_idx] * (1.0 - frac) + envelope[env_idx + 1] * frac
                    } else {
                        envelope[env_idx]
                    }
                } else {
                    1.0
                };

                let final_magnitude = residual * shifted_envelope;
                synthesis_magnitudes[new_bin] += final_magnitude;
                synthesis_frequencies[new_bin] =
                    analysis_frequencies[i] * pitch_shift_ratio * octave_factor;
            }
        }

        // Synthesis phase reconstruction
        frequency_analysis::perform_phase_vocoder_synthesis::<N>(
            &mut synthesis_magnitudes,
            &mut synthesis_frequencies,
            last_output_phases,
            &mut full_spectrum,
            hop_size,
        );
    }

    // Inverse FFT
    let time_domain_result = F::inverse_fft(&mut full_spectrum);
    let mut output_samples = [0.0f32; N];

    let playing_note = note != 0;
    for i in 0..N {
        let vocals = time_domain_result[i].re;
        let synth = if let Some(ref synth_buf) = synth_buffer {
            synth_buf[i]
        } else {
            0.0
        };
        let mixed = if playing_note {
            vocals * 0.96 + synth * 0.04
        } else {
            vocals
        };
        output_samples[i] = mixed * analysis_window_buffer[i];
    }

    output_samples
}

/// Generic harmony generation based on notes provided
pub fn process_harmony_generic<const N: usize, const HALF_N: usize, F>(
    unwrapped_buffer: &mut [f32; N],
    last_input_phases: &mut [f32; N],
    last_output_phases: &mut [f32; N],
    config: &VocalEffectsConfig,
    settings: &MusicalSettings,
) -> [f32; N]
where
    F: FftOps<N, HALF_N>,
{
    let hop_size = (N as f32 * config.hop_ratio) as usize;
    let bin_width = config.sample_rate / N as f32;

    let analysis_window_buffer = F::get_hann_window();
    let mut full_spectrum: [microfft::Complex32; N] = [microfft::Complex32 { re: 0.0, im: 0.0 }; N];
    let mut analysis_magnitudes = [0.0; HALF_N];
    let mut analysis_frequencies = [0.0; HALF_N];
    let mut synthesis_magnitudes = [0.0; N];
    let mut synthesis_frequencies = [0.0; N];
    let mut envelope = [1.0f32; HALF_N];
    let mut hps_buffer = [1.0f32; HALF_N];

    // Apply windowing
    for i in 0..N {
        unwrapped_buffer[i] *= analysis_window_buffer[i];
    }

    // Forward FFT
    let fft_result = F::forward_fft(unwrapped_buffer);

    // Analysis phase
    frequency_analysis::perform_phase_vocoder_analysis::<N, HALF_N>(
        fft_result,
        last_input_phases,
        &mut analysis_magnitudes,
        &mut analysis_frequencies,
        hop_size,
    );

    // Extract formant envelope for more natural sound
    extract_simple_envelope::<HALF_N>(&analysis_magnitudes, &mut envelope);

    let fundamental_bin = frequency_analysis::find_fundamental_frequency(
        &analysis_magnitudes,
        &mut hps_buffer,
        config,
    );
    let input_freq = analysis_frequencies[fundamental_bin] * bin_width;

    // Zero synthesis arrays
    synthesis_magnitudes.fill(0.0);
    synthesis_frequencies.fill(0.0);

    let mut voices = 0;

    // Add original voice
    for (s, &a) in synthesis_magnitudes[..HALF_N].iter_mut().zip(analysis_magnitudes.iter()) {
        *s += a;
    }
    synthesis_frequencies[..HALF_N].copy_from_slice(&analysis_frequencies[..HALF_N]);
    voices += 1;

    // Add pitch-shifted harmonies
    for &frequency in settings.midi_frequencies.iter() {
        if frequency == 0.0 {
            continue;
        }

        voices += 1;

        let shift_ratio = frequency / input_freq;

        for i in 0..HALF_N {
            if analysis_magnitudes[i] <= config.magnitude_threshold {
                continue;
            }

            // Calculate new bin position for this harmony
            let new_bin_f = i as f32 * shift_ratio;
            let new_bin = floorf(new_bin_f + 0.5) as usize;

            if new_bin < HALF_N {
                // Initial spectral envelope position
                let preserved_envelope = envelope[i];

                // Accumulate magnitude for this harmony voice
                synthesis_magnitudes[new_bin] += analysis_magnitudes[i] * preserved_envelope;

                // For harmonies, we can just use the shifted frequency
                synthesis_frequencies[new_bin] = analysis_frequencies[i] * shift_ratio;
            }
        }
    }

    // Normalize by voice count to prevent clipping
    let normalization = 1.0 / voices as f32;
    for s in synthesis_magnitudes[..HALF_N].iter_mut() {
        *s *= normalization;
    }

    // Synthesis phase reconstruction
    frequency_analysis::perform_phase_vocoder_synthesis::<N>(
        &mut synthesis_magnitudes,
        &mut synthesis_frequencies,
        last_output_phases,
        &mut full_spectrum,
        hop_size,
    );

    let time_domain_result = F::inverse_fft(&mut full_spectrum);

    let mut output_samples = [0.0f32; N];

    for i in 0..N {
        let mut sample = time_domain_result[i].re;
        sample *= analysis_window_buffer[i];

        // Optional: soft clipping for safety
        if sample.abs() > 0.95 {
            let sign = if sample >= 0.0 { 1.0 } else { -1.0 };
            let compressed = 0.95 - 0.05 * expf(-fabsf(sample));
            sample = sign * compressed;
        }

        output_samples[i] = sample;
    }

    output_samples
}

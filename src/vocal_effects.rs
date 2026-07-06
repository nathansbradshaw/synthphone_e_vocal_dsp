//! Core Vocal Effects Implementation
//!
//! This module contains shared vocal effects processing functions that use generics
//! to eliminate code duplication across different FFT size configurations.

use crate::{
    MusicalSettings, ProcessingMode, VocalEffectsConfig,
    dsp::{Fft512, Fft1024, Fft2048, Fft4096, FftOps},
    effects::{
        process_dry_generic, process_harmony_generic, process_harmony_generic_with_formant,
        process_pitch_correction_generic, process_vocode_generic,
    },
};

/// Generic vocal effects processing function that works with different FFT sizes and processing modes
#[allow(clippy::too_many_arguments)]
fn process_vocal_effects<const N: usize, const HALF_N: usize, F>(
    unwrapped_buffer: &mut [f32; N],
    carrier_buffer: Option<&mut [f32; N]>,
    last_input_phases: &mut [f32; N],
    last_output_phases: &mut [f32; N],
    cached_envelope: &mut [f32; HALF_N],
    cached_inv_envelope: &mut [f32; HALF_N],
    previous_pitch_shift_ratio: f32,
    config: &VocalEffectsConfig,
    settings: &MusicalSettings,
) -> [f32; N]
where
    F: FftOps<N, HALF_N>,
{
    match settings.mode {
        ProcessingMode::PitchControl => process_pitch_correction_generic::<N, HALF_N, F>(
            unwrapped_buffer,
            last_input_phases,
            last_output_phases,
            previous_pitch_shift_ratio,
            cached_envelope,
            cached_inv_envelope,
            config,
            settings,
        ),
        ProcessingMode::Vocode => process_vocode_generic::<N, HALF_N, F>(
            unwrapped_buffer,
            carrier_buffer.expect("Carrier buffer required for vocode mode"),
            last_input_phases,
            last_output_phases,
            config,
            settings,
        ),
        ProcessingMode::Dry => process_dry_generic::<N, HALF_N, F>(
            unwrapped_buffer,
            carrier_buffer,
            last_input_phases,
            last_output_phases,
            cached_envelope,
            cached_inv_envelope,
            config,
            settings,
        ),
        ProcessingMode::Harmony => process_harmony_generic::<N, HALF_N, F>(
            unwrapped_buffer,
            last_input_phases,
            last_output_phases,
            config,
            settings,
        ),
    }
}

/// Specialized vocal effects function for 512-point FFT.
/// `cached_envelope` and `cached_inv_envelope` are RTIC locals holding the formant
/// envelope for whichever mode is active; PitchControl/Dry recompute them fresh
/// every hop (they're the sole voice, so staleness would be audible), they're just
/// reused scratch space here to avoid a per-call stack allocation.
#[allow(clippy::too_many_arguments)]
pub fn process_vocal_effects_512(
    unwrapped_buffer: &mut [f32; 512],
    carrier_buffer: Option<&mut [f32; 512]>,
    last_input_phases: &mut [f32; 512],
    last_output_phases: &mut [f32; 512],
    cached_envelope: &mut [f32; 256],
    cached_inv_envelope: &mut [f32; 256],
    previous_pitch_shift_ratio: f32,
    config: &VocalEffectsConfig,
    settings: &MusicalSettings,
) -> [f32; 512] {
    process_vocal_effects::<512, 256, Fft512>(
        unwrapped_buffer,
        carrier_buffer,
        last_input_phases,
        last_output_phases,
        cached_envelope,
        cached_inv_envelope,
        previous_pitch_shift_ratio,
        config,
        settings,
    )
}

/// Specialized vocal effects function for 1024-point FFT.
/// `cached_envelope` and `cached_inv_envelope` are RTIC locals holding the formant
/// envelope for whichever mode is active. Harmony caches across hops via
/// `process_harmony_generic_with_formant`'s own refresh cadence (safe there — it
/// only colors a secondary voice blended with the untouched original); PitchControl
/// and Dry recompute fresh every hop since they're the sole voice in the path.
#[allow(clippy::too_many_arguments)]
pub fn process_vocal_effects_1024(
    unwrapped_buffer: &mut [f32; 1024],
    carrier_buffer: Option<&mut [f32; 1024]>,
    last_input_phases: &mut [f32; 1024],
    last_output_phases: &mut [f32; 1024],
    cached_envelope: &mut [f32; 512],
    cached_inv_envelope: &mut [f32; 512],
    previous_pitch_shift_ratio: f32,
    config: &VocalEffectsConfig,
    settings: &MusicalSettings,
) -> [f32; 1024] {
    if settings.mode == ProcessingMode::Harmony {
        process_harmony_generic_with_formant::<1024, 512, Fft1024>(
            unwrapped_buffer,
            last_input_phases,
            last_output_phases,
            cached_envelope,
            cached_inv_envelope,
            config,
            settings,
        )
    } else {
        process_vocal_effects::<1024, 512, Fft1024>(
            unwrapped_buffer,
            carrier_buffer,
            last_input_phases,
            last_output_phases,
            cached_envelope,
            cached_inv_envelope,
            previous_pitch_shift_ratio,
            config,
            settings,
        )
    }
}

/// Specialized vocal effects function for 2048-point FFT
#[allow(clippy::too_many_arguments)]
pub fn process_vocal_effects_2048(
    unwrapped_buffer: &mut [f32; 2048],
    carrier_buffer: Option<&mut [f32; 2048]>,
    last_input_phases: &mut [f32; 2048],
    last_output_phases: &mut [f32; 2048],
    cached_envelope: &mut [f32; 1024],
    cached_inv_envelope: &mut [f32; 1024],
    previous_pitch_shift_ratio: f32,
    config: &VocalEffectsConfig,
    settings: &MusicalSettings,
) -> [f32; 2048] {
    process_vocal_effects::<2048, 1024, Fft2048>(
        unwrapped_buffer,
        carrier_buffer,
        last_input_phases,
        last_output_phases,
        cached_envelope,
        cached_inv_envelope,
        previous_pitch_shift_ratio,
        config,
        settings,
    )
}

/// Specialized vocal effects function for 4096-point FFT
#[allow(clippy::too_many_arguments)]
pub fn process_vocal_effects_4096(
    unwrapped_buffer: &mut [f32; 4096],
    carrier_buffer: Option<&mut [f32; 4096]>,
    last_input_phases: &mut [f32; 4096],
    last_output_phases: &mut [f32; 4096],
    cached_envelope: &mut [f32; 2048],
    cached_inv_envelope: &mut [f32; 2048],
    previous_pitch_shift_ratio: f32,
    config: &VocalEffectsConfig,
    settings: &MusicalSettings,
) -> [f32; 4096] {
    process_vocal_effects::<4096, 2048, Fft4096>(
        unwrapped_buffer,
        carrier_buffer,
        last_input_phases,
        last_output_phases,
        cached_envelope,
        cached_inv_envelope,
        previous_pitch_shift_ratio,
        config,
        settings,
    )
}

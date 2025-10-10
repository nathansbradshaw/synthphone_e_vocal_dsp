//! Core Vocal Effects Implementation
//!
//! This module contains shared vocal effects processing functions that use generics
//! to eliminate code duplication across different FFT size configurations.

use crate::{
    MusicalSettings, ProcessingMode, VocalEffectsConfig,
    dsp::{Fft512, Fft1024, Fft2048, Fft4096, FftOps},
    effects::{process_dry_generic, process_pitch_correction_generic, process_vocode_generic, process_harmony_generic},
};

/// Generic vocal effects processing function that works with different FFT sizes and processing modes
fn process_vocal_effects<const N: usize, const HALF_N: usize, F>(
    unwrapped_buffer: &mut [f32; N],
    carrier_buffer: Option<&mut [f32; N]>,
    last_input_phases: &mut [f32; N],
    last_output_phases: &mut [f32; N],
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
        ProcessingMode::Phone => process_dry_generic::<N, HALF_N, F>(
            unwrapped_buffer,
            carrier_buffer,
            last_input_phases,
            last_output_phases,
            config,
            settings,
        ),
    }
}

/// Specialized vocal effects function for 512-point FFT
pub fn process_vocal_effects_512(
    unwrapped_buffer: &mut [f32; 512],
    carrier_buffer: Option<&mut [f32; 512]>,
    last_input_phases: &mut [f32; 512],
    last_output_phases: &mut [f32; 512],
    previous_pitch_shift_ratio: f32,
    config: &VocalEffectsConfig,
    settings: &MusicalSettings,
) -> [f32; 512] {
    process_vocal_effects::<512, 256, Fft512>(
        unwrapped_buffer,
        carrier_buffer,
        last_input_phases,
        last_output_phases,
        previous_pitch_shift_ratio,
        config,
        settings,
    )
}

/// Specialized vocal effects function for 1024-point FFT
pub fn process_vocal_effects_1024(
    unwrapped_buffer: &mut [f32; 1024],
    carrier_buffer: Option<&mut [f32; 1024]>,
    last_input_phases: &mut [f32; 1024],
    last_output_phases: &mut [f32; 1024],
    previous_pitch_shift_ratio: f32,
    config: &VocalEffectsConfig,
    settings: &MusicalSettings,
) -> [f32; 1024] {
    process_vocal_effects::<1024, 512, Fft1024>(
        unwrapped_buffer,
        carrier_buffer,
        last_input_phases,
        last_output_phases,
        previous_pitch_shift_ratio,
        config,
        settings,
    )
}

/// Specialized vocal effects function for 2048-point FFT
pub fn process_vocal_effects_2048(
    unwrapped_buffer: &mut [f32; 2048],
    carrier_buffer: Option<&mut [f32; 2048]>,
    last_input_phases: &mut [f32; 2048],
    last_output_phases: &mut [f32; 2048],
    previous_pitch_shift_ratio: f32,
    config: &VocalEffectsConfig,
    settings: &MusicalSettings,
) -> [f32; 2048] {
    process_vocal_effects::<2048, 1024, Fft2048>(
        unwrapped_buffer,
        carrier_buffer,
        last_input_phases,
        last_output_phases,
        previous_pitch_shift_ratio,
        config,
        settings,
    )
}

/// Specialized vocal effects function for 4096-point FFT
pub fn process_vocal_effects_4096(
    unwrapped_buffer: &mut [f32; 4096],
    carrier_buffer: Option<&mut [f32; 4096]>,
    last_input_phases: &mut [f32; 4096],
    last_output_phases: &mut [f32; 4096],
    previous_pitch_shift_ratio: f32,
    config: &VocalEffectsConfig,
    settings: &MusicalSettings,
) -> [f32; 4096] {
    process_vocal_effects::<4096, 2048, Fft4096>(
        unwrapped_buffer,
        carrier_buffer,
        last_input_phases,
        last_output_phases,
        previous_pitch_shift_ratio,
        config,
        settings,
    )
}

//! Core Vocal Effects Implementation
//!
//! This module contains shared vocal effects processing functions that use generics
//! to eliminate code duplication across different FFT size configurations.

use crate::{
    MusicalSettings, ProcessingMode, VocalEffectsConfig,
    dsp::{Fft512, Fft1024, Fft2048, Fft4096, FftOps},
    effects::{
        process_dry_generic, process_harmony_generic, process_harmony_generic_with_formant,
        process_pitch_correction_generic, process_pitch_correction_generic_with_state,
        process_vocode_generic,
    },
};

/// Allocation-free streaming state. Each MIDI harmony retains a coherent phase offset.
pub struct VocalStreamState {
    correction_ratio: f32,
    harmony: crate::effects::harmony_stream::HarmonyState,
}

impl Default for VocalStreamState {
    fn default() -> Self {
        Self::new()
    }
}
impl VocalStreamState {
    pub const fn new() -> Self {
        Self {
            correction_ratio: 1.0,
            harmony: crate::effects::harmony_stream::HarmonyState::new(),
        }
    }
    pub fn reset(&mut self) {
        self.correction_ratio = 1.0;
        self.harmony.reset();
    }
}

#[allow(clippy::too_many_arguments)]
#[inline(never)]
pub fn process_vocal_effects_1024_stream(
    input: &mut [f32; 1024],
    carrier: Option<&mut [f32; 1024]>,
    input_phases: &mut [f32; 1024],
    output_phases: &mut [f32; 1024],
    envelope: &mut [f32; 512],
    inv_envelope: &mut [f32; 512],
    state: &mut VocalStreamState,
    config: &VocalEffectsConfig,
    settings: &MusicalSettings,
) -> [f32; 1024] {
    if settings.mode == ProcessingMode::Harmony {
        state.correction_ratio = 1.0;
        crate::effects::harmony_stream::process(
            input,
            input_phases,
            envelope,
            inv_envelope,
            &mut state.harmony,
            config,
            settings,
        )
    } else {
        if settings.mode != ProcessingMode::PitchControl {
            state.harmony.reset();
        }
        // Dispatch directly: the legacy dispatcher also contains the old
        // harmony scratch arrays, which otherwise inflate every mode's stack.
        if settings.mode != ProcessingMode::PitchControl {
            state.correction_ratio = 1.0;
        }
        match settings.mode {
            ProcessingMode::PitchControl => {
                state.harmony.clear_voices();
                let source_pitch = state.harmony.track_pitch(input, config);
                crate::effects::pitch_correction::<1024, 512, Fft1024>(
                    input,
                    input_phases,
                    output_phases,
                    state.correction_ratio,
                    Some(&mut state.correction_ratio),
                    Some(source_pitch),
                    config,
                    settings,
                )
            }
            ProcessingMode::Dry => process_dry_generic::<1024, 512, Fft1024>(
                input,
                carrier,
                input_phases,
                output_phases,
                config,
                settings,
            ),
            ProcessingMode::Vocode => process_vocode_generic::<1024, 512, Fft1024>(
                input,
                carrier.expect("Carrier buffer required for vocode mode"),
                input_phases,
                output_phases,
                config,
                settings,
            ),
            ProcessingMode::Harmony => unreachable!(),
        }
    }
}

/// Streaming 1024-point entry point. Retains the correction ratio between hops;
/// leaving PitchControl releases it. Legacy by-value entry points remain available.
#[allow(clippy::too_many_arguments)]
pub fn process_vocal_effects_1024_with_state(
    input: &mut [f32; 1024],
    carrier: Option<&mut [f32; 1024]>,
    input_phases: &mut [f32; 1024],
    output_phases: &mut [f32; 1024],
    envelope: &mut [f32; 512],
    inv_envelope: &mut [f32; 512],
    ratio: &mut f32,
    config: &VocalEffectsConfig,
    settings: &MusicalSettings,
) -> [f32; 1024] {
    if settings.mode == ProcessingMode::PitchControl {
        process_pitch_correction_generic_with_state::<1024, 512, Fft1024>(
            input,
            input_phases,
            output_phases,
            ratio,
            config,
            settings,
        )
    } else {
        *ratio = 1.0;
        process_vocal_effects_1024(
            input,
            carrier,
            input_phases,
            output_phases,
            envelope,
            inv_envelope,
            *ratio,
            config,
            settings,
        )
    }
}

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

/// Specialized vocal effects function for 1024-point FFT.
/// In Harmony mode, `cached_harmony_envelope` and `cached_harmony_inv_envelope`
/// are RTIC locals that persist the spectral envelope across hops.
#[allow(clippy::too_many_arguments)]
pub fn process_vocal_effects_1024(
    unwrapped_buffer: &mut [f32; 1024],
    carrier_buffer: Option<&mut [f32; 1024]>,
    last_input_phases: &mut [f32; 1024],
    last_output_phases: &mut [f32; 1024],
    cached_harmony_envelope: &mut [f32; 512],
    cached_harmony_inv_envelope: &mut [f32; 512],
    previous_pitch_shift_ratio: f32,
    config: &VocalEffectsConfig,
    settings: &MusicalSettings,
) -> [f32; 1024] {
    if settings.mode == ProcessingMode::Harmony {
        process_harmony_generic_with_formant::<1024, 512, Fft1024>(
            unwrapped_buffer,
            last_input_phases,
            last_output_phases,
            cached_harmony_envelope,
            cached_harmony_inv_envelope,
            config,
            settings,
        )
    } else {
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

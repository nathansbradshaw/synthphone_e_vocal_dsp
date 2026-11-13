use synthphone_e_vocal_dsp::{
    MusicalSettings, ProcessingMode, VocalEffectsConfig, VocalStreamState,
    dsp::{Fft1024, FftOps},
    process_vocal_effects_1024_stream,
};

fn frame(mix: f32, carrier: bool) -> [f32; 1024] {
    let mut input =
        core::array::from_fn(|i| 0.2 * (core::f32::consts::TAU * 200.0 * i as f32 / 48000.0).sin());
    let mut carrier = core::array::from_fn(|i| {
        if carrier {
            2.0 * ((330.0 * i as f32 / 48000.0) % 1.0) - 1.0
        } else {
            0.0
        }
    });
    process_vocal_effects_1024_stream(
        &mut input,
        Some(&mut carrier),
        &mut [0.0; 1024],
        &mut [0.0; 1024],
        &mut [1.0; 512],
        &mut [1.0; 512],
        &mut VocalStreamState::new(),
        &VocalEffectsConfig { vocoder_mix: mix, ..Default::default() },
        &MusicalSettings { mode: ProcessingMode::Vocode, ..Default::default() },
    )
}

#[test]
fn zero_amount_keeps_the_original_voice_even_without_a_carrier() {
    let output = frame(0.0, false);
    let window = Fft1024::get_hann_window();
    for (i, &sample) in output.iter().enumerate() {
        let original = 0.2 * (core::f32::consts::TAU * 200.0 * i as f32 / 48000.0).sin();
        assert!((sample - original * window[i] * window[i]).abs() < 1e-6);
    }
}

#[test]
fn middle_amount_is_a_time_aligned_blend_of_dry_and_full_vocoder() {
    let dry = frame(0.0, true);
    let wet = frame(1.0, true);
    let middle = frame(0.5, true);
    assert!(dry.iter().zip(wet).any(|(&a, b)| (a - b).abs() > 0.01));
    for ((&a, b), actual) in dry.iter().zip(wet).zip(middle) {
        assert!((actual - (a + b) * 0.5).abs() < 1e-6);
    }
}

use synthphone_e_vocal_dsp::{
    MusicalSettings, VocalEffectsConfig, VocalStreamState, process_vocal_effects_1024_stream,
};

fn frame(input: &mut [f32; 1024], strength: f32) -> [f32; 1024] {
    process_vocal_effects_1024_stream(
        input,
        None,
        &mut [0.0; 1024],
        &mut [0.0; 1024],
        &mut [1.0; 512],
        &mut [1.0; 512],
        &mut VocalStreamState::new(),
        &VocalEffectsConfig { pitch_correction_strength: strength, ..Default::default() },
        &MusicalSettings::default(),
    )
}

#[test]
fn rejected_aperiodic_input_is_independent_of_correction_strength() {
    let mut seed = 1234567_u32;
    let input = core::array::from_fn(|_| {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        (seed as f32 / u32::MAX as f32 * 0.2) - 0.1
    });
    let bypass = frame(&mut input.clone(), 0.0);
    let corrected = frame(&mut input.clone(), 1.0);
    assert_eq!(bypass, corrected, "an unvoiced frame must not receive a made-up pitch");
    assert!(corrected.iter().any(|x| x.abs() > 0.01), "unvoiced audio must pass, not mute");
}

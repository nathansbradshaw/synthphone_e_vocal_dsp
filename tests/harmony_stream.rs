use synthphone_e_vocal_dsp::{
    MusicalSettings, ProcessingMode, VocalEffectsConfig, VocalStreamState,
    process_vocal_effects_1024_stream,
};

#[test]
fn midi_only_stream_has_no_ungated_lead() {
    let mut state = VocalStreamState::new();
    let config = VocalEffectsConfig { harmony_lead_gain: 0.0, ..Default::default() };
    let settings = MusicalSettings { mode: ProcessingMode::Harmony, ..Default::default() };
    let (mut phases, mut output_phases) = ([0.0; 1024], [0.0; 1024]);
    let (mut envelope, mut inverse) = ([1.0; 512], [1.0; 512]);
    for hop in 0..12 {
        let mut input = core::array::from_fn(|i| {
            libm::sinf((i + hop * 256) as f32 * core::f32::consts::TAU * 200.0 / 48000.0) * 0.2
        });
        let output = process_vocal_effects_1024_stream(
            &mut input,
            None,
            &mut phases,
            &mut output_phases,
            &mut envelope,
            &mut inverse,
            &mut state,
            &config,
            &settings,
        );
        assert!(output.iter().all(|&s| s == 0.0));
    }
    state.reset();
}

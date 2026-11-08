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

#[test]
fn invalid_midi_gains_are_silent_and_never_poison_the_audio() {
    for gain in [f32::NAN, f32::INFINITY, -1.0] {
        let mut state = VocalStreamState::new();
        let config = VocalEffectsConfig { harmony_lead_gain: 0.0, ..Default::default() };
        let mut settings = MusicalSettings { mode: ProcessingMode::Harmony, ..Default::default() };
        settings.midi_frequencies[0] = 220.0;
        settings.midi_gains[0] = gain;
        let (mut phases, mut output_phases) = ([0.0; 1024], [0.0; 1024]);
        let (mut envelope, mut inverse) = ([1.0; 512], [1.0; 512]);
        for hop in 0..16 {
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
            assert!(output.iter().all(|s| s.is_finite() && s.abs() < 1e-7));
        }
    }
}

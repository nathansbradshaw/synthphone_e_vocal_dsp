use synthphone_e_vocal_dsp::audio::{find_nearest_note_in_key, get_frequency};

#[test]
fn note_boundaries_are_symmetric_in_semitones() {
    let notes: [f32; 2] = [261.625_58, 293.664_76];
    let middle = (notes[0] * notes[1]).sqrt();
    let cent = 2_f32.powf(1.0 / 1200.0);
    assert_eq!(find_nearest_note_in_key(middle / cent, &notes), notes[0]);
    assert_eq!(find_nearest_note_in_key(middle * cent, &notes), notes[1]);
}

#[test]
fn absent_or_invalid_notes_never_generate_a_frequency() {
    for note in [i8::MIN, -1, 0, 13, i8::MAX] {
        assert_eq!(get_frequency(0, note, 1.0, false), 0.0);
    }
    for ratio in [0.0, -1.0, f32::NAN, f32::INFINITY] {
        assert_eq!(get_frequency(0, 1, ratio, false), 0.0);
    }
    assert_eq!(find_nearest_note_in_key(440.0, &[]), 0.0);
    for input in [0.0, -1.0, f32::NAN, f32::INFINITY] {
        assert_eq!(find_nearest_note_in_key(input, &[440.0]), 0.0);
    }
}

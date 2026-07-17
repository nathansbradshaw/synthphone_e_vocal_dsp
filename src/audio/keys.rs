use crate::audio::frequencies::*;

/// A KeyScale is simply an array of 7 static string slices, e.g. ["C", "D", "E", "F", "G", "A", "B"].
pub type KeyScaleFrequencies = [f32; 70];
pub type KeyScale = ([&'static str; 7], KeyScaleFrequencies);

/// =======================================
/// Major Key Scales
/// =======================================
pub const C_MAJOR_SCALE: KeyScale =
    (["C", "D", "E", "F", "G", "A", "B"], C_MAJOR_SCALE_FREQUENCIES);
pub const G_MAJOR_SCALE: KeyScale =
    (["G", "A", "B", "C", "D", "E", "F#"], G_MAJOR_SCALE_FREQUENCIES);
pub const D_MAJOR_SCALE: KeyScale =
    (["D", "E", "F#", "G", "A", "B", "C#"], D_MAJOR_SCALE_FREQUENCIES);
pub const A_MAJOR_SCALE: KeyScale =
    (["A", "B", "C#", "D", "E", "F#", "G#"], A_MAJOR_SCALE_FREQUENCIES);
pub const E_MAJOR_SCALE: KeyScale =
    (["E", "F#", "G#", "A", "B", "C#", "D#"], E_MAJOR_SCALE_FREQUENCIES);
pub const B_MAJOR_SCALE: KeyScale =
    (["B", "C#", "D#", "E", "F#", "G#", "A#"], B_MAJOR_SCALE_FREQUENCIES);
pub const F_SHARP_MAJOR_SCALE: KeyScale =
    (["F#", "G#", "A#", "B", "C#", "D#", "E#"], FS_MAJOR_SCALE_FREQUENCIES);
pub const C_SHARP_MAJOR_SCALE: KeyScale =
    (["C#", "D#", "E#", "F#", "G#", "A#", "B#"], CS_MAJOR_SCALE_FREQUENCIES);
pub const F_MAJOR_SCALE: KeyScale =
    (["F", "G", "A", "Bb", "C", "D", "E"], F_MAJOR_SCALE_FREQUENCIES);
pub const BB_MAJOR_SCALE: KeyScale =
    (["Bb", "C", "D", "Eb", "F", "G", "A"], BB_MAJOR_SCALE_FREQUENCIES);
pub const EB_MAJOR_SCALE: KeyScale =
    (["Eb", "F", "G", "Ab", "Bb", "C", "D"], EB_MAJOR_SCALE_FREQUENCIES);
pub const AB_MAJOR_SCALE: KeyScale =
    (["Ab", "Bb", "C", "Db", "Eb", "F", "G"], AB_MAJOR_SCALE_FREQUENCIES);

/// =======================================
/// Minor Key Scales (Natural Minor)
/// =======================================
pub const A_MINOR_SCALE: KeyScale =
    (["A", "B", "C", "D", "E", "F", "G"], A_MINOR_SCALE_FREQUENCIES);
pub const E_MINOR_SCALE: KeyScale =
    (["E", "F#", "G", "A", "B", "C", "D"], E_MINOR_SCALE_FREQUENCIES);
pub const B_MINOR_SCALE: KeyScale =
    (["B", "C#", "D", "E", "F#", "G", "A"], B_MINOR_SCALE_FREQUENCIES);
pub const F_SHARP_MINOR_SCALE: KeyScale =
    (["F#", "G#", "A", "B", "C#", "D", "E"], FS_MINOR_SCALE_FREQUENCIES);
pub const C_SHARP_MINOR_SCALE: KeyScale =
    (["C#", "D#", "E", "F#", "G#", "A", "B"], CS_MINOR_SCALE_FREQUENCIES);
pub const G_SHARP_MINOR_SCALE: KeyScale =
    (["G#", "A#", "B", "C#", "D#", "E", "F#"], AB_MINOR_SCALE_FREQUENCIES);
pub const D_MINOR_SCALE: KeyScale =
    (["D", "E", "F", "G", "A", "Bb", "C"], D_MINOR_SCALE_FREQUENCIES);
pub const G_MINOR_SCALE: KeyScale =
    (["G", "A", "Bb", "C", "D", "Eb", "F"], G_MINOR_SCALE_FREQUENCIES);
pub const C_MINOR_SCALE: KeyScale =
    (["C", "D", "Eb", "F", "G", "Ab", "Bb"], C_MINOR_SCALE_FREQUENCIES);
pub const F_MINOR_SCALE: KeyScale =
    (["F", "G", "Ab", "Bb", "C", "Db", "Eb"], F_MINOR_SCALE_FREQUENCIES);
pub const BB_MINOR_SCALE: KeyScale =
    (["Bb", "C", "Db", "Eb", "F", "Gb", "Ab"], BB_MINOR_SCALE_FREQUENCIES);
pub const EB_MINOR_SCALE: KeyScale =
    (["Eb", "F", "Gb", "Ab", "Bb", "Cb", "Db"], EB_MINOR_SCALE_FREQUENCIES);

/// =======================================
/// All 24 Keys in One Array
/// =======================================
/// Each tuple is (KeyScale, KeyName).
pub const KEYS: [(KeyScale, &str); 24] = [
    (C_MAJOR_SCALE, "C"),
    (G_MAJOR_SCALE, "G"),
    (D_MAJOR_SCALE, "D"),
    (A_MAJOR_SCALE, "A"),
    (E_MAJOR_SCALE, "E"),
    (B_MAJOR_SCALE, "B"),
    (F_SHARP_MAJOR_SCALE, "F#"),
    (C_SHARP_MAJOR_SCALE, "C#"),
    (F_MAJOR_SCALE, "F"),
    (BB_MAJOR_SCALE, "Bb"),
    (EB_MAJOR_SCALE, "Eb"),
    (AB_MAJOR_SCALE, "Ab"),
    (A_MINOR_SCALE, "A"),
    (E_MINOR_SCALE, "E"),
    (B_MINOR_SCALE, "B"),
    (F_SHARP_MINOR_SCALE, "F#"),
    (C_SHARP_MINOR_SCALE, "C#"),
    (G_SHARP_MINOR_SCALE, "G#"),
    (D_MINOR_SCALE, "D"),
    (G_MINOR_SCALE, "G"),
    (C_MINOR_SCALE, "C"),
    (F_MINOR_SCALE, "F"),
    (BB_MINOR_SCALE, "Bb"),
    (EB_MINOR_SCALE, "Eb"),
];

/// Returns the note name from a given `scale` based on `note` (1..9).
/// Wraps around at 8 and 9, which effectively map back to scale.0\[0\] or scale.0\[1\].
pub fn get_note_name(note: i8, scale: KeyScale) -> &'static str {
    match note {
        1 => scale.0[0],
        2 => scale.0[1],
        3 => scale.0[2],
        4 => scale.0[3],
        5 => scale.0[4],
        6 => scale.0[5],
        7 => scale.0[6],
        8 => scale.0[0],  // wrap around
        9 => scale.0[1],  // wrap around
        10 => scale.0[2], // wrap around
        11 => scale.0[3], // wrap around
        12 => scale.0[4], // wrap around
        _ => "",          // out of range
    }
}

/// Returns one of the 24 `KEYS` based on `key` (0..23).
/// Defaults to `KEYS[0]` (C Major) if out of range.
pub fn get_key(key: i8) -> KeyScale {
    if let Some(k) = KEYS.get(key as usize) {
        k.0
    } else {
        // Fallback to first key (C Major) if out of range
        KEYS[0].0
    }
}

/// Returns the "mode" of the key: "Major" if index < 12, otherwise "Minor".
/// Defaults to "Major" if out of range.
pub fn get_mode_name(key: i8) -> &'static str {
    let idx = key as usize;
    if idx < 24 {
        // If index is in 0..11, it's Major, else Minor.
        if idx < 12 { "Major" } else { "Minor" }
    } else {
        // Out of range => default "Major"
        "Major"
    }
}

/// Returns the name of the key based on KEY_NAMES
/// Defaults to "C Major" if out of range.
pub fn get_key_name(key: i8) -> &'static str {
    let key = key as usize;
    if key < KEYS.len() {
        KEYS[key].1
    } else {
        // Fallback to first key, or handle differently
        KEYS[0].1
    }
}

pub fn get_scale_by_key(key: i8) -> &'static KeyScaleFrequencies {
    let key = key as usize;
    if key < KEYS.len() {
        &KEYS[key].0.1
    } else {
        // Fallback to first key, or handle differently
        &KEYS[0].0.1
    }
}

/// Returns the frequency of `note` in `key`, shifted by `octave_ratio`
/// (0.5 = octave down, 1.0 = unchanged, 2.0 = octave up). The note is looked
/// up at a fixed reference row of the scale table and the ratio is applied
/// continuously, so shifts between whole octaves are equally valid.
pub fn get_frequency(key: i8, note: i8, octave_ratio: f32, is_vocoder: bool) -> f32 {
    let offset = if is_vocoder { 0 } else { 2 };
    let reference_row = 2 + offset;
    let note_index = reference_row * 7 + note as usize - 1;

    // out-of-bounds check
    if key as usize >= KEYS.len() {
        return 0.0;
    }

    KEYS[key as usize].0.1[note_index] * octave_ratio
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The old `get_frequency` selected a scale-table row per octave value
    /// (1/2/4 → one row down / reference / one row up). Adjacent rows are
    /// built from the same base frequencies with the octave multiplier
    /// doubled, so multiplying the reference row by 0.5/1.0/2.0 must
    /// reproduce the old rows exactly — bit for bit, since multiplying an
    /// f32 by a power of two only changes the exponent.
    #[test]
    fn test_ratio_reproduces_old_octave_rows() {
        for key in 0..24i8 {
            let scale = get_scale_by_key(key);
            for note in 1..=7usize {
                for (is_vocoder, offset) in [(true, 0usize), (false, 2usize)] {
                    let row = |r: usize| scale[r * 7 + note - 1];

                    // Old behavior: octave 1 → row 1+offset, 2 → 2+offset, 4 → 3+offset
                    assert_eq!(get_frequency(key, note as i8, 0.5, is_vocoder), row(1 + offset));
                    assert_eq!(get_frequency(key, note as i8, 1.0, is_vocoder), row(2 + offset));
                    assert_eq!(get_frequency(key, note as i8, 2.0, is_vocoder), row(3 + offset));
                }
            }
        }
    }

    /// Ratios between whole octaves return frequencies strictly between the
    /// old table rows — values the row lookup could never produce (it
    /// returned 0.0 for anything but octave 1/2/4).
    #[test]
    fn test_in_between_ratios_land_between_old_rows() {
        // Equal-temperament ratios: 2^(semitones/12)
        const UP_3_ST: f32 = 1.189_207_1;
        const UP_7_ST: f32 = 1.498_307_1;
        const DOWN_5_ST: f32 = 0.749_153_55;

        for key in 0..24i8 {
            for note in 1..=7i8 {
                for is_vocoder in [true, false] {
                    let down_octave = get_frequency(key, note, 0.5, is_vocoder);
                    let reference = get_frequency(key, note, 1.0, is_vocoder);
                    let up_octave = get_frequency(key, note, 2.0, is_vocoder);

                    let up3 = get_frequency(key, note, UP_3_ST, is_vocoder);
                    let up7 = get_frequency(key, note, UP_7_ST, is_vocoder);
                    let down5 = get_frequency(key, note, DOWN_5_ST, is_vocoder);

                    assert!(reference < up3 && up3 < up_octave);
                    assert!(reference < up7 && up7 < up_octave);
                    assert!(up3 < up7, "+3 st must sit below +7 st");
                    assert!(down_octave < down5 && down5 < reference);
                }
            }
        }

        // Musical sanity: the root shifted up a perfect fifth (+7 st) must
        // land on the 5th scale degree of the same key. C Major is key 0;
        // note 1 = C, note 5 = G.
        let c_up_fifth = get_frequency(0, 1, UP_7_ST, true);
        let g = get_frequency(0, 5, 1.0, true);
        let rel_err = (c_up_fifth - g).abs() / g;
        assert!(
            rel_err < 1e-3,
            "C shifted +7 semitones should equal G: {c_up_fifth} Hz vs {g} Hz"
        );
    }
}

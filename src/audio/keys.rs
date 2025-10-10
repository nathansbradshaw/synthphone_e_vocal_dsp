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
        8 => scale.0[0], // wrap around
        9 => scale.0[1], // wrap around
        _ => "",         // out of range
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

pub fn get_frequency(key: i8, note: i8, octave: i8, is_vocoder: bool) -> f32 {
    //TODO: maybe i should have the octave store index insted of values so i don't have to convert here?
    let offset = if is_vocoder { 0 } else { 2 };

    let octave_idx = match octave {
        1 => 1 + offset, // first row
        2 => 2 + offset, // second row
        4 => 3 + offset, // third row
        _ => return 0.0, // invalid flag
    };

    let note_index = octave_idx * 7 + note as usize - 1;

    // out-of-bounds check
    if key as usize >= KEYS.len() {
        return 0.0;
    }

    KEYS[key as usize].0.1[note_index]
}

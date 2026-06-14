pub const C_MAJOR_SCALE_STEPS: [usize; 7] = [0, 2, 4, 5, 7, 9, 11];
pub const MAX_OCTAVES: usize = 10;

pub const NOTE_NAMES: [&str; 12] =
    ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"];
pub const BASE_FREQUENCIES: [f32; 12] = [
    16.35, 17.32, 18.35, 19.45, 20.60, 21.83, 23.12, 24.50, 25.96, 27.50, 29.14, 30.87,
];
pub const MAJOR_SCALE_STEPS: [usize; 7] = [2, 2, 1, 2, 2, 2, 1]; // W-W-H-W-W-W-H
pub const MINOR_SCALE_STEPS: [usize; 7] = [2, 1, 2, 2, 1, 2, 2]; // W-H-W-W-H-W-W

// Define the number of octaves (same as C_MAJOR_SCALE_FREQUENCIES length)
pub const SCALE_NOTES: usize = 7;

// Const function to generate major scale frequencies
const fn generate_major_scale_frequencies(root_index: usize) -> [f32; SCALE_NOTES * MAX_OCTAVES] {
    let mut frequencies = [0.0; SCALE_NOTES * MAX_OCTAVES];
    let mut freq_index = 0;

    let mut octave = 0;
    while octave < MAX_OCTAVES {
        let mut current_index = root_index;
        let mut step_index = 0;

        while step_index < MAJOR_SCALE_STEPS.len() && freq_index < SCALE_NOTES * MAX_OCTAVES {
            let octave_multiplier = pow_f32(2.0, octave as i32);
            frequencies[freq_index] = BASE_FREQUENCIES[current_index] * octave_multiplier;
            freq_index += 1;

            current_index = (current_index + MAJOR_SCALE_STEPS[step_index]) % 12;
            step_index += 1;
        }
        octave += 1;
    }

    frequencies
}

// Const function to generate minor scale frequencies
const fn generate_minor_scale_frequencies(root_index: usize) -> [f32; SCALE_NOTES * MAX_OCTAVES] {
    let mut frequencies = [0.0; SCALE_NOTES * MAX_OCTAVES];
    let mut freq_index = 0;

    let mut octave = 0;
    while octave < MAX_OCTAVES {
        let mut current_index = root_index;
        let mut step_index = 0;

        while step_index < MINOR_SCALE_STEPS.len() && freq_index < SCALE_NOTES * MAX_OCTAVES {
            let octave_multiplier = pow_f32(2.0, octave as i32);
            frequencies[freq_index] = BASE_FREQUENCIES[current_index] * octave_multiplier;
            freq_index += 1;

            current_index = (current_index + MINOR_SCALE_STEPS[step_index]) % 12;
            step_index += 1;
        }
        octave += 1;
    }

    frequencies
}

// Const function to compute power of f32 (simplified for integer exponents)
const fn pow_f32(base: f32, exp: i32) -> f32 {
    if exp == 0 {
        return 1.0;
    }
    if exp < 0 {
        return 1.0 / pow_f32(base, -exp);
    }

    let mut result = 1.0;
    let mut i = 0;
    while i < exp {
        result *= base;
        i += 1;
    }
    result
}

// Generate all scale frequencies at compile time
pub const C_MAJOR_SCALE_FREQUENCIES: [f32; 70] = generate_major_scale_frequencies(0); // C = index 0
pub const CS_MAJOR_SCALE_FREQUENCIES: [f32; 70] = generate_major_scale_frequencies(1); // C# = index 1
pub const D_MAJOR_SCALE_FREQUENCIES: [f32; 70] = generate_major_scale_frequencies(2); // D = index 2
pub const EB_MAJOR_SCALE_FREQUENCIES: [f32; 70] = generate_major_scale_frequencies(3); // D#/Eb = index 3
pub const E_MAJOR_SCALE_FREQUENCIES: [f32; 70] = generate_major_scale_frequencies(4); // E = index 4
pub const F_MAJOR_SCALE_FREQUENCIES: [f32; 70] = generate_major_scale_frequencies(5); // F = index 5
pub const FS_MAJOR_SCALE_FREQUENCIES: [f32; 70] = generate_major_scale_frequencies(6); // F#/Gb = index 6
pub const G_MAJOR_SCALE_FREQUENCIES: [f32; 70] = generate_major_scale_frequencies(7); // G = index 7
pub const AB_MAJOR_SCALE_FREQUENCIES: [f32; 70] = generate_major_scale_frequencies(8); // G#/Ab = index 8
pub const A_MAJOR_SCALE_FREQUENCIES: [f32; 70] = generate_major_scale_frequencies(9); // A = index 9
pub const BB_MAJOR_SCALE_FREQUENCIES: [f32; 70] = generate_major_scale_frequencies(10); // A#/Bb = index 10
pub const B_MAJOR_SCALE_FREQUENCIES: [f32; 70] = generate_major_scale_frequencies(11); // B = index 11

pub const C_MINOR_SCALE_FREQUENCIES: [f32; 70] = generate_minor_scale_frequencies(0); // C = index 0
pub const CS_MINOR_SCALE_FREQUENCIES: [f32; 70] = generate_minor_scale_frequencies(1); // C# = index 1
pub const D_MINOR_SCALE_FREQUENCIES: [f32; 70] = generate_minor_scale_frequencies(2); // D = index 2
pub const EB_MINOR_SCALE_FREQUENCIES: [f32; 70] = generate_minor_scale_frequencies(3); // D#/Eb = index 3
pub const E_MINOR_SCALE_FREQUENCIES: [f32; 70] = generate_minor_scale_frequencies(4); // E = index 4
pub const F_MINOR_SCALE_FREQUENCIES: [f32; 70] = generate_minor_scale_frequencies(5); // F = index 5
pub const FS_MINOR_SCALE_FREQUENCIES: [f32; 70] = generate_minor_scale_frequencies(6); // F#/Gb = index 6
pub const G_MINOR_SCALE_FREQUENCIES: [f32; 70] = generate_minor_scale_frequencies(7); // G = index 7
pub const AB_MINOR_SCALE_FREQUENCIES: [f32; 70] = generate_minor_scale_frequencies(8); // G#/Ab = index 8
pub const A_MINOR_SCALE_FREQUENCIES: [f32; 70] = generate_minor_scale_frequencies(9); // A = index 9
pub const BB_MINOR_SCALE_FREQUENCIES: [f32; 70] = generate_minor_scale_frequencies(10); // A#/Bb = index 10
pub const B_MINOR_SCALE_FREQUENCIES: [f32; 70] = generate_minor_scale_frequencies(11); // B = index 11

// Combined frequencies array for all scales
pub const FREQUENCIES: [&[f32]; 24] = [
    // Major scales
    &C_MAJOR_SCALE_FREQUENCIES,
    &CS_MAJOR_SCALE_FREQUENCIES,
    &D_MAJOR_SCALE_FREQUENCIES,
    &EB_MAJOR_SCALE_FREQUENCIES,
    &E_MAJOR_SCALE_FREQUENCIES,
    &F_MAJOR_SCALE_FREQUENCIES,
    &FS_MAJOR_SCALE_FREQUENCIES,
    &G_MAJOR_SCALE_FREQUENCIES,
    &AB_MAJOR_SCALE_FREQUENCIES,
    &A_MAJOR_SCALE_FREQUENCIES,
    &BB_MAJOR_SCALE_FREQUENCIES,
    &B_MAJOR_SCALE_FREQUENCIES,
    // Minor scales
    &C_MINOR_SCALE_FREQUENCIES,
    &CS_MINOR_SCALE_FREQUENCIES,
    &D_MINOR_SCALE_FREQUENCIES,
    &EB_MINOR_SCALE_FREQUENCIES,
    &E_MINOR_SCALE_FREQUENCIES,
    &F_MINOR_SCALE_FREQUENCIES,
    &FS_MINOR_SCALE_FREQUENCIES,
    &G_MINOR_SCALE_FREQUENCIES,
    &AB_MINOR_SCALE_FREQUENCIES,
    &A_MINOR_SCALE_FREQUENCIES,
    &BB_MINOR_SCALE_FREQUENCIES,
    &B_MINOR_SCALE_FREQUENCIES,
];

pub fn find_nearest_note_frequency(input_frequency: f32) -> f32 {
    let mut nearest_frequency = C_MAJOR_SCALE_FREQUENCIES[0];
    let mut min_difference = (input_frequency - nearest_frequency).abs();

    for &scale in &FREQUENCIES {
        for &frequency in scale {
            let difference = (input_frequency - frequency).abs();
            if difference < min_difference {
                min_difference = difference;
                nearest_frequency = frequency;
            }
        }
    }

    nearest_frequency
}

pub fn find_nearest_note_in_key(input_frequency: f32, key_frequencies: &[f32]) -> f32 {
    let mut nearest_frequency = key_frequencies[0];
    let mut min_difference = (input_frequency - nearest_frequency).abs();

    for &frequency in key_frequencies {
        let difference = (input_frequency - frequency).abs();
        if difference < min_difference {
            min_difference = difference;
            nearest_frequency = frequency;
        }
    }

    nearest_frequency
}

#[cfg(test)]
mod tests {
    use super::*;

    pub fn generate_major_scale_frequencies_runtime(
        root_note: &str,
    ) -> [f32; SCALE_NOTES * MAX_OCTAVES] {
        let mut frequencies = [0.0; SCALE_NOTES * MAX_OCTAVES];

        // Find the root note index
        let root_index = NOTE_NAMES
            .iter()
            .position(|&note| note == root_note)
            .expect("Invalid root note");

        let mut freq_index = 0;

        for octave in 0..MAX_OCTAVES {
            let mut current_index = root_index;
            let mut current_frequency =
                BASE_FREQUENCIES[current_index] * (2.0f32).powi(octave as i32);

            for &step in MAJOR_SCALE_STEPS.iter() {
                if freq_index < SCALE_NOTES * MAX_OCTAVES {
                    frequencies[freq_index] = current_frequency;
                    freq_index += 1;
                }
                current_index = (current_index + step) % 12;
                current_frequency = BASE_FREQUENCIES[current_index] * (2.0f32).powi(octave as i32);
            }
        }

        frequencies
    }

    #[test]
    fn test_compile_time_generation_matches_runtime() {
        // Test that compile-time generated frequencies match runtime generation
        let runtime_c_major = generate_major_scale_frequencies_runtime("C");

        // Compare first few values to ensure they match
        for i in 0..10 {
            assert!((C_MAJOR_SCALE_FREQUENCIES[i] - runtime_c_major[i]).abs() < 0.01);
        }
    }

    #[test]
    fn test_find_nearest_note_frequency_exact_match() {
        let result = find_nearest_note_frequency(440.0);
        assert!((result - 440.0).abs() < 0.01);
    }

    #[test]
    fn test_find_nearest_note_frequency_in_between() {
        let result = find_nearest_note_frequency(450.0);
        assert!(result > 400.0 && result < 500.0);
    }

    #[test]
    fn test_find_nearest_note_frequency_below_range() {
        let result = find_nearest_note_frequency(10.0);
        assert!(result > 15.0);
    }

    #[test]
    fn test_find_nearest_note_frequency_above_range() {
        let result = find_nearest_note_frequency(20000.0);
        assert!(result < 20000.0);
    }

    #[test]
    fn test_find_nearest_note_frequency_mid_point() {
        let result = find_nearest_note_frequency(247.0);
        assert!(result > 200.0 && result < 300.0);
    }

    #[test]
    fn test_find_nearest_note_frequency_edge_case_low() {
        let result = find_nearest_note_frequency(16.0);
        assert!(result > 15.0);
    }

    #[test]
    fn test_find_nearest_note_frequency_edge_case_high() {
        let result = find_nearest_note_frequency(16000.0);
        assert!(result > 10000.0);
    }

    #[test]
    fn test_find_nearest_note_frequency_very_close_lower() {
        let result = find_nearest_note_frequency(439.9);
        assert!((result - 440.0).abs() < 1.0);
    }

    #[test]
    fn test_find_nearest_note_frequency_very_close_upper() {
        let result = find_nearest_note_frequency(440.1);
        assert!((result - 440.0).abs() < 1.0);
    }
}

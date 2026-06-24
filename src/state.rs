/// Processing modes for vocal effects
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ProcessingMode {
    /// Pitch correction/PitchControl mode
    PitchControl,
    /// Vocoder mode - applies vocal formants to carrier signal
    Vocode,
    /// Dry mode - pitch shifting with formant preservation but no correction
    Dry,
    /// Harmony mode - adding harmonies based on the played notes
    Harmony,
}

/// Musical settings for vocal effects processing
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MusicalSettings {
    /// Musical key (0-23, see keys module for mapping)
    pub key: i8,
    /// Specific note (0 = auto mode, 1-12 = specific note in scale)
    pub note: i8,
    /// Midi notes provided
    pub midi_frequencies: [f32; 8],
    /// Octave setting
    pub octave: i8,
    /// Formant shift mode (0 = none, 1 = male, 2 = female)
    pub formant: i8,
    /// Spectral envelope warp ratio for male mode — lower shifts formants down
    pub formant_male_ratio: f32,
    /// Spectral envelope warp ratio for female mode — higher shifts formants up
    pub formant_female_ratio: f32,
    /// Processing mode for vocal effects
    pub mode: ProcessingMode,
}

impl Default for MusicalSettings {
    fn default() -> Self {
        Self {
            key: 0,
            note: 0,
            midi_frequencies: [0.0; 8],
            octave: 2,
            formant: 0,
            formant_male_ratio: 0.5,
            formant_female_ratio: 2.0,
            mode: ProcessingMode::PitchControl,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_musical_settings_default() {
        let settings = MusicalSettings::default();
        assert_eq!(settings.key, 0);
        assert_eq!(settings.note, 0);
        assert_eq!(settings.octave, 2);
        assert_eq!(settings.formant, 0);
    }

    #[test]
    fn test_formant_male_ratio_default() {
        let settings = MusicalSettings::default();
        assert!(
            (settings.formant_male_ratio - 0.5).abs() < 1e-6,
            "default male ratio should be 0.5, got {}",
            settings.formant_male_ratio
        );
    }

    #[test]
    fn test_formant_female_ratio_default() {
        let settings = MusicalSettings::default();
        assert!(
            (settings.formant_female_ratio - 2.0).abs() < 1e-6,
            "default female ratio should be 2.0, got {}",
            settings.formant_female_ratio
        );
    }

    #[test]
    fn test_formant_ratios_can_be_customised() {
        let settings = MusicalSettings {
            formant_male_ratio: 0.25,
            formant_female_ratio: 3.5,
            ..MusicalSettings::default()
        };
        assert!((settings.formant_male_ratio - 0.25).abs() < 1e-6);
        assert!((settings.formant_female_ratio - 3.5).abs() < 1e-6);
    }
}

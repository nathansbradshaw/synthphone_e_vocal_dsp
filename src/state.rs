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
    /// Phone mode - ringer drum sounds (or regular phone notes not sure yet)
    Phone,
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
    /// Formant shift mode (0 = none, 1 = lower, 2 = higher)
    pub formant: i8,
    /// Processing mode for vocal effects
    pub mode: ProcessingMode,
}

impl Default for MusicalSettings {
    fn default() -> Self {
        Self {
            key: 0,  // C Major
            note: 0, // Auto mode
            midi_frequencies: [0.0; 8],
            octave: 2,
            formant: 0, // No formant shift
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
}

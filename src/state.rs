/// Processing modes for vocal effects
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ProcessingMode {
    /// Pitch correction/autotune mode
    Autotune,
    /// Vocoder mode - applies vocal formants to carrier signal
    Vocode,
    /// Dry mode - pitch shifting with formant preservation but no correction
    Dry,
}

/// Musical settings for vocal effects processing
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MusicalSettings {
    /// Musical key (0-23, see keys module for mapping)
    pub key: i8,
    /// Specific note (0 = auto mode, 1-9 = specific note in scale)
    pub note: i8,
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
            octave: 2,
            formant: 0, // No formant shift
            mode: ProcessingMode::Autotune,
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

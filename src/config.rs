//! Configuration types for the vocal effects library
/// Configuration for the vocal effects processor
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VocalEffectsConfig {
    /// FFT size (must be power of 2, between 512-4096)
    pub fft_size: usize,
    /// Hop size for overlap-add processing
    pub hop_size: usize,
    /// Sample rate in Hz
    pub sample_rate: f32,
    /// Hop ratio as fraction of FFT size (0.0625 to 0.5)
    pub hop_ratio: f32,
    /// Speed of pitch correction transition (0.0 to 1.0)
    pub transition_speed: f32,
    /// Strength of pitch correction (0.0 to 1.0, closer to 1.0 = stronger)
    pub pitch_correction_strength: f32,
    /// Minimum frequency to process (Hz)
    pub min_frequency: f32,
    /// Maximum frequency to process (Hz)
    pub max_frequency: f32,
    /// Noise floor ratio for HPS fundamental detection (0.0–1.0; bins below this fraction of peak are ignored)
    pub hps_noise_floor_ratio: f32,
    /// Downsample factors used in HPS algorithm (up to 4 factors; unused slots should be 0)
    pub hps_downsample_factors: [usize; 4],
    /// Number of active downsample factors in hps_downsample_factors
    pub hps_num_factors: usize,
    /// Magnitude threshold below which bins are skipped during processing
    pub magnitude_threshold: f32,
}

impl Default for VocalEffectsConfig {
    fn default() -> Self {
        Self {
            fft_size: 1024,
            hop_size: 256, // Will be calculated from hop_ratio
            sample_rate: 48000.0,
            hop_ratio: 0.25,
            transition_speed: 0.1,
            pitch_correction_strength: 0.999,
            min_frequency: 50.0,
            max_frequency: 4000.0,
            hps_noise_floor_ratio: 0.1,
            hps_downsample_factors: [2, 3, 4, 0],
            hps_num_factors: 3,
            magnitude_threshold: 1e-6,
        }
    }
}

impl VocalEffectsConfig {
    /// Create a new configuration with validation
    pub fn new(
        fft_size: usize,
        sample_rate: f32,
        hop_ratio: f32,
    ) -> Result<Self, crate::VocalEffectsError> {
        if !fft_size.is_power_of_two() {
            return Err(crate::VocalEffectsError::InvalidConfiguration);
        }
        if !(512..=4096).contains(&fft_size) {
            return Err(crate::VocalEffectsError::InvalidConfiguration);
        }
        if sample_rate <= 0.0 {
            return Err(crate::VocalEffectsError::InvalidConfiguration);
        }
        if !(0.0625..=0.5).contains(&hop_ratio) {
            return Err(crate::VocalEffectsError::InvalidConfiguration);
        }

        let hop_size = (fft_size as f32 * hop_ratio) as usize;

        Ok(Self { fft_size, hop_size, sample_rate, hop_ratio, ..Default::default() })
    }

    /// Update hop ratio and recalculate hop size
    pub fn set_hop_ratio(&mut self, hop_ratio: f32) -> Result<(), crate::VocalEffectsError> {
        if !(0.0625..=0.5).contains(&hop_ratio) {
            return Err(crate::VocalEffectsError::InvalidConfiguration);
        }
        self.hop_ratio = hop_ratio;
        self.hop_size = (self.fft_size as f32 * hop_ratio) as usize;
        Ok(())
    }

    /// Get the bin width in Hz
    pub fn bin_width(&self) -> f32 {
        self.sample_rate / self.fft_size as f32
    }

    /// Get the spectrum size (FFT size / 2)
    pub fn spectrum_size(&self) -> usize {
        self.fft_size / 2
    }
}

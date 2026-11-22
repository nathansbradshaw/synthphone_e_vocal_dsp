pub struct Oscillator {
    pub freq: f32,
    sample_rate: f32,
    phase: f32,
    waveform: Waveform,
}

#[derive(Copy, Clone)]
pub enum Waveform {
    Sine,
    Saw,
    Square,
    Triangle,
}

impl Oscillator {
    pub fn new(freq: f32, sample_rate: f32, waveform: Waveform) -> Self {
        Self { freq, sample_rate, phase: 0.0, waveform }
    }

    pub fn set_waveform(&mut self, waveform: Waveform) {
        self.waveform = waveform;
    }

    pub fn set_freq(&mut self, freq: f32) {
        self.freq = freq;
    }

    pub fn next_value(&mut self) -> f32 {
        let phase_inc = self.freq / self.sample_rate;
        self.phase += phase_inc;
        if self.phase >= 1.0 {
            self.phase -= 1.0;
        }

        match self.waveform {
            Waveform::Sine => {
                let angle = core::f32::consts::TAU * self.phase;
                if (0.0..=core::f32::consts::TAU).contains(&angle) {
                    crate::dsp::trigonometry::sin_cos(angle).0
                } else {
                    // Preserve the public oscillator's behavior outside its
                    // usual positive, sub-Nyquist instrument frequency range.
                    libm::sinf(angle)
                }
            }
            Waveform::Saw => 2.0 * self.phase - 1.0,
            Waveform::Square => {
                if self.phase < 0.5 {
                    1.0
                } else {
                    -1.0
                }
            }
            Waveform::Triangle => 4.0 * (self.phase - 0.5).abs() - 1.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sine_table_preserves_pitch_phase_and_amplitude_across_instrument_range() {
        for rate in [48_014.312f32, 48_187.487] {
            for frequency in [29.14, 110., 440., 2093., 12543.85, 14080.] {
                let mut oscillator = Oscillator::new(frequency, rate, Waveform::Sine);
                let mut reference_phase = 0f32;
                let mut squared_error = 0f64;
                for _ in 0..96000 {
                    reference_phase += frequency / rate;
                    if reference_phase >= 1. {
                        reference_phase -= 1.;
                    }
                    let reference = libm::sinf(core::f32::consts::TAU * reference_phase);
                    let actual = oscillator.next_value();
                    assert_eq!(oscillator.phase, reference_phase);
                    let error = (actual - reference).abs();
                    assert!(error < 5e-6, "{rate} Hz / {frequency} Hz: {error}");
                    squared_error += f64::from(error).powi(2);
                }
                assert!(squared_error / 96000. < 9e-12);
            }
        }
    }
    #[test]
    fn unusual_oscillator_phases_keep_the_libm_fallback() {
        for frequency in [-220., 192000., f32::INFINITY, f32::NAN] {
            let mut oscillator = Oscillator::new(frequency, 48000., Waveform::Sine);
            for _ in 0..20 {
                let actual = oscillator.next_value();
                let reference = libm::sinf(core::f32::consts::TAU * oscillator.phase);
                assert!(
                    actual.is_nan() && reference.is_nan()
                        || actual.to_bits() == reference.to_bits()
                );
            }
        }
    }
}

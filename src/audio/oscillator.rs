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
            Waveform::Sine => libm::sinf(2.0 * core::f32::consts::PI * self.phase),
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

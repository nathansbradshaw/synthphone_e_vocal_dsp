//! Polyphonic vocal resynthesis: coherent harmonic regions, one shared IFFT.
//!
//! Bins belonging to a vocal harmonic move together, retaining their complex
//! phase relationships. One accumulated fundamental phase offset per voice
//! keeps its harmonics coherent as the sung pitch changes. Symmetric-window
//! centre correction avoids cancellation during fractional-bin interpolation.
//! Adapted to monophonic vocals from Laroche and Dolson's frequency-domain
//! harmonizing method: <https://www.ee.columbia.edu/~dpwe/papers/LaroD99-pvoc.pdf>
use crate::{
    MusicalSettings, VocalEffectsConfig,
    dsp::{Fft1024, FftOps, frequency_analysis, wrap_phase},
};
use core::f32::consts::TAU;
use libm::{floorf, roundf};

pub const HARMONY_VOICES: usize = 8;

// A quarter-wave table gives 256 samples per turn using 260 bytes of flash.
// Curvature-corrected interpolation has <5e-6 absolute error; its cost is
// bounded and small enough for independent voices at every FFT hop on M7.
const fn quarter_wave() -> [f32; 65] {
    let mut table = [0.0; 65];
    let mut i = 0;
    while i <= 64 {
        let x = i as f64 * core::f64::consts::FRAC_PI_2 / 64.0;
        let x2 = x * x;
        table[i] = (x
            * (1.0
                + x2 * (-1.0 / 6.0
                    + x2 * (1.0 / 120.0
                        + x2 * (-1.0 / 5040.0
                            + x2 * (1.0 / 362880.0
                                + x2 * (-1.0 / 39916800.0 + x2 / 6227020800.0)))))))
            as f32;
        i += 1;
    }
    table
}
static QUARTER: [f32; 65] = quarter_wave();

#[inline(always)]
fn table_sin(index: usize) -> f32 {
    let index = index & 255;
    let offset = index & 63;
    match index >> 6 {
        0 => QUARTER[offset],
        1 => QUARTER[64 - offset],
        2 => -QUARTER[offset],
        _ => -QUARTER[64 - offset],
    }
}

/// Input is the sum of two phases in [-pi,pi], hence in [-tau,tau].
#[inline(always)]
pub(super) fn sin_cos(phase: f32) -> (f32, f32) {
    let positive = if phase < 0.0 { phase + TAU } else { phase };
    let position = positive * (256.0 / TAU);
    let index = position as usize;
    let fraction = position - index as f32;
    let s = table_sin(index);
    let c = table_sin(index + 64);
    // Correct the leading chord-interpolation error using sin'' = -sin.
    let curvature = 1.0 + 0.000_301_196_4 * fraction * (1.0 - fraction);
    (
        (s + fraction * (table_sin(index + 1) - s)) * curvature,
        (c + fraction * (table_sin(index + 65) - c)) * curvature,
    )
}

pub struct HarmonyState {
    phases: [f32; HARMONY_VOICES],
    targets: [f32; HARMONY_VOICES],
    initialized: [bool; HARMONY_VOICES],
    active: bool,
    pitch_history: [f32; 2048],
    source_pitch: f32,
    unvoiced_hops: u8,
}

impl Default for HarmonyState {
    fn default() -> Self {
        Self::new()
    }
}

impl HarmonyState {
    pub const fn new() -> Self {
        Self {
            phases: [0.0; HARMONY_VOICES],
            targets: [0.0; HARMONY_VOICES],
            initialized: [false; HARMONY_VOICES],
            active: false,
            pitch_history: [0.0; 2048],
            source_pitch: 0.0,
            unvoiced_hops: 0,
        }
    }
    pub fn reset(&mut self) {
        if self.active {
            self.phases.fill(0.0);
            self.targets.fill(0.0);
            self.initialized.fill(false);
            self.active = false;
            self.pitch_history.fill(0.0);
            self.source_pitch = 0.0;
            self.unvoiced_hops = 0;
        }
    }

    // Pitch Control reuses the input period history without retaining Harmony
    // voice assignments. `active` also marks that history as needing reset.
    pub(crate) fn clear_voices(&mut self) {
        self.phases.fill(0.0);
        self.targets.fill(0.0);
        self.initialized.fill(false);
        self.source_pitch = 0.0;
        self.unvoiced_hops = 0;
    }

    pub(crate) fn track_pitch(&mut self, input: &[f32], config: &VocalEffectsConfig) -> f32 {
        let hop = (input.len() as f32 * config.hop_ratio) as usize;
        self.pitch_history.copy_within(hop.., 0);
        self.pitch_history[2048 - hop..].copy_from_slice(&input[input.len() - hop..]);
        self.active = true;
        frequency_analysis::find_pitch_harmony(&self.pitch_history, config)
    }

    fn assign(&mut self, frequencies: &[f32; 8]) -> ([usize; 8], [bool; 8], usize) {
        let mut wanted = [0.0; 8];
        let mut count = 0;
        for &f in frequencies {
            if f.is_finite() && f > 0.0 && !wanted[..count].contains(&f) {
                wanted[count] = f;
                count += 1;
            }
        }
        wanted[..count].sort_unstable_by(f32::total_cmp);
        let mut slots = [usize::MAX; 8];
        let mut used = [false; 8];

        // Keep held notes in their own phase bank when other notes change.
        for i in 0..count {
            if let Some(slot) = (0..8).find(|&s| !used[s] && self.targets[s] == wanted[i]) {
                slots[i] = slot;
                used[slot] = true;
            }
        }
        for i in 0..count {
            if slots[i] != usize::MAX {
                continue;
            }
            let slot = (0..8).find(|&s| !used[s]).unwrap();
            if self.targets[slot] == 0.0 {
                self.initialized[slot] = false;
            }
            self.targets[slot] = wanted[i];
            slots[i] = slot;
            used[slot] = true;
        }
        for (slot, &is_used) in used.iter().enumerate() {
            if !is_used {
                self.targets[slot] = 0.0;
                self.initialized[slot] = false;
            }
        }
        self.active = true;
        (slots, self.initialized.map(|ready| !ready), count)
    }
}

#[allow(clippy::too_many_arguments)]
// Keep harmony scratch out of the caller's stack frame in other effect modes.
#[inline(never)]
pub fn process(
    input: &mut [f32; 1024],
    input_phases: &mut [f32; 1024],
    envelope: &mut [f32; 512],
    inv_envelope: &mut [f32; 512],
    state: &mut HarmonyState,
    config: &VocalEffectsConfig,
    settings: &MusicalSettings,
) -> [f32; 1024] {
    process_with_fft::<1024, 512, Fft1024>(
        input,
        input_phases,
        envelope,
        inv_envelope,
        state,
        config,
        settings,
    )
}

/// The same harmonizer with a selectable analysis window. Callers must supply
/// overlap-add normalization for their chosen window/hop and retain all state.
/// The device entry point above continues to use 1024 samples.
#[allow(clippy::too_many_arguments)]
#[inline(never)]
pub fn process_with_fft<const N: usize, const HALF: usize, F: FftOps<N, HALF>>(
    input: &mut [f32; N],
    input_phases: &mut [f32; N],
    envelope: &mut [f32; HALF],
    inv_envelope: &mut [f32; HALF],
    state: &mut HarmonyState,
    config: &VocalEffectsConfig,
    settings: &MusicalSettings,
) -> [f32; N] {
    let hop = (N as f32 * config.hop_ratio) as usize;
    let mut fundamental = state.track_pitch(input, config);
    let window = F::get_hann_window();
    for (sample, &w) in input.iter_mut().zip(window) {
        *sample *= w;
    }
    let fft = F::forward_fft(input);
    let mut spectrum = [microfft::Complex32 { re: 0.0, im: 0.0 }; N];
    let mut magnitudes = [0.0; HALF];
    let mut frequencies = [0.0; HALF];
    frequency_analysis::perform_phase_vocoder_analysis::<N, HALF>(
        fft,
        input_phases,
        &mut magnitudes,
        &mut frequencies,
        hop,
    );
    // The complete period identifies the register. Reinterpreting it from
    // even/odd spectral balance makes a vowel change look like an octave jump.
    // The period window lags fast slides and can reject them entirely. Follow
    // the current spectral phase using the last instantaneous register, while
    // allowing the complete period to identify a true large register change.
    let previous = state.source_pitch;
    let reference = if previous > 0.0
        && (fundamental == 0.0 || (0.7..1.4).contains(&(fundamental / previous)))
    {
        previous
    } else {
        fundamental
    };
    if reference > 0.0 {
        let peak = (1..HALF).max_by(|&a, &b| magnitudes[a].total_cmp(&magnitudes[b])).unwrap();
        let measured = frequencies[peak] * config.sample_rate / N as f32;
        let harmonic = roundf(measured / reference).max(1.0);
        let refined = measured / harmonic;
        let power = magnitudes.iter().map(|m| m * m).sum::<f32>();
        // White noise normally has only ~1% of its energy in its strongest bin.
        // Do not turn an unvoiced consonant into a remembered pitched voice.
        let tonal = magnitudes[peak] > config.magnitude_threshold
            && magnitudes[peak] * magnitudes[peak] > 0.04 * power;
        if tonal && (refined / reference - 1.0).abs() < 0.12 {
            fundamental = refined;
        }
    }
    if fundamental > 0.0 {
        state.source_pitch = fundamental;
        state.unvoiced_hops = 0;
    } else {
        state.unvoiced_hops = state.unvoiced_hops.saturating_add(1);
        if state.unvoiced_hops > 48 {
            state.source_pitch = 0.0;
        }
    }

    // Refresh the O(N) envelope each hop. A silence-cached envelope must not
    // erase the start of a word for up to the old 85ms refresh interval.
    let width = N / 64;
    let mut sum: f32 = magnitudes[..=width].iter().sum();
    let (mut lo, mut hi) = (0, width);
    for i in 0..HALF {
        envelope[i] = sum / (hi - lo + 1) as f32;
        inv_envelope[i] = 1.0 / envelope[i].max(1e-6);
        if hi + 1 < HALF {
            hi += 1;
            sum += magnitudes[hi];
        }
        if lo + width < i + 1 {
            sum -= magnitudes[lo];
            lo += 1;
        }
    }
    let (slots, initialize, count) = state.assign(&settings.midi_frequencies);
    let mut voice_gains = [0.0_f32; 8];
    for &slot in &slots[..count] {
        for i in 0..8 {
            let gain = settings.midi_gains[i];
            if settings.midi_frequencies[i] == state.targets[slot] && gain.is_finite() {
                voice_gains[slot] = voice_gains[slot].max(gain.clamp(0.0, 8.0));
            }
        }
    }
    let voiced = fundamental.is_finite() && fundamental > 0.001;
    let voice_count = if voiced { count } else { 0 };
    let lead = if config.harmony_lead_gain.is_finite() {
        config.harmony_lead_gain.clamp(0.0, 1.0)
    } else {
        1.0
    };
    let normalization = 1.0 / (lead + voice_count as f32).max(1.0);
    let dry_gain = if !voiced && count > 0 {
        // Noise/consonants are shared once. Match the RMS scale of the averaged
        // independent voiced parts instead of jumping to unity at every dropout.
        if lead == 0.0 && slots[..count].iter().all(|&slot| voice_gains[slot] == 1.0) {
            1.0 / libm::sqrtf(count as f32)
        } else {
            let power = slots[..count]
                .iter()
                .map(|&slot| voice_gains[slot] * voice_gains[slot])
                .sum::<f32>();
            libm::sqrtf(lead * lead + power) / (lead + count as f32)
        }
    } else {
        lead * normalization
    };
    for i in 0..HALF {
        spectrum[i].re = fft[i].re * dry_gain;
        spectrum[i].im = if i == 0 { 0.0 } else { fft[i].im * dry_gain };
    }

    if voiced {
        let bin_hz = config.sample_rate / N as f32;
        let centre_phase = core::f32::consts::PI * ((N - 1) as f32 / N as f32);
        for &slot in &slots[..count] {
            let difference = state.targets[slot] - fundamental;
            if initialize[slot] {
                state.phases[slot] = 0.0;
            }
            state.phases[slot] =
                wrap_phase(state.phases[slot] + TAU * difference * hop as f32 / config.sample_rate);
            if voice_gains[slot] == 0.0 {
                state.initialized[slot] = true;
                continue;
            }
            let mut previous_harmonic = 0;
            let (mut shift, mut fraction) = (0_i32, 0.0);
            let (mut low_rotation, mut high_rotation) = ((0.0, 0.0), (0.0, 0.0));
            for i in 1..HALF {
                if magnitudes[i] <= config.magnitude_threshold {
                    continue;
                }
                // Phase-derived frequency keeps a low vocal's whole FFT lobe
                // together. Bin centres alone can split a 90 Hz tone into
                // differently shifted partials. Retain bin-centre ownership for
                // invalid estimates or values beyond a two-bin neighbourhood.
                let measured = frequencies[i];
                let position =
                    if measured.is_finite() && measured > 0.0 && (measured - i as f32).abs() <= 2.0
                    {
                        measured
                    } else {
                        i as f32
                    };
                let harmonic = (roundf(position * bin_hz / fundamental) as usize).max(1);
                let delta = harmonic as f32 * difference / bin_hz;
                if !delta.is_finite() || delta.abs() > N as f32 {
                    continue;
                }
                if harmonic != previous_harmonic {
                    shift = floorf(delta) as i32;
                    fraction = delta - shift as f32;
                    // Translate the whole harmonic lobe, preserving its complex
                    // phase relationships. Interpolate around the window centre
                    // so opposite-phase neighbouring FFT bins do not cancel.
                    let phase = harmonic as f32 * state.phases[slot] - shift as f32 * centre_phase;
                    low_rotation = sin_cos(wrap_phase(phase));
                    high_rotation = sin_cos(wrap_phase(phase - centre_phase));
                    previous_harmonic = harmonic;
                }
                for (offset, weight, (sin, cos)) in
                    [(0, 1.0 - fraction, low_rotation), (1, fraction, high_rotation)]
                {
                    let destination = i as i64 + shift as i64 + offset;
                    let bin = destination.unsigned_abs() as usize;
                    if bin >= HALF || weight <= 0.0 {
                        continue;
                    }
                    let gain = inv_envelope[i]
                        * envelope[bin]
                        * weight
                        * normalization
                        * voice_gains[slot];
                    let re = (fft[i].re * cos - fft[i].im * sin) * gain;
                    let im = (fft[i].re * sin + fft[i].im * cos) * gain;
                    spectrum[bin].re += re;
                    spectrum[bin].im += if destination < 0 { -im } else { im };
                }
            }
            state.initialized[slot] = true;
        }
    }
    // IFFT is linear, so adding independently phased complex voices first
    // preserves their separate pitches without one IFFT per harmony voice.
    for bin in 1..HALF {
        spectrum[N - bin] = spectrum[bin].conj();
    }
    spectrum[0].im = 0.0;
    let time = F::inverse_fft(&mut spectrum);
    let mut output = [0.0; N];
    for i in 0..N {
        output[i] = time[i].re * window[i];
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn interpolated_phases_stay_below_five_millionths_error() {
        for i in -16384..=16384 {
            let phase = i as f32 * TAU / 16384.0;
            let (s, c) = sin_cos(phase);
            assert!((s - libm::sinf(phase)).abs() < 5e-6, "sin({phase})");
            assert!((c - libm::cosf(phase)).abs() < 5e-6, "cos({phase})");
        }
    }
}

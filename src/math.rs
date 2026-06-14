//! Mathematical utilities

use libm::expf;

/// Clamp a value between min and max
#[inline(always)]
pub fn clamp(value: f32, min: f32, max: f32) -> f32 {
    if value < min {
        min
    } else if value > max {
        max
    } else {
        value
    }
}

/// Linear interpolation between two values
#[inline(always)]
pub fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + t * (b - a)
}

/// Check if a number is a power of two
#[inline(always)]
pub fn is_power_of_two(n: usize) -> bool {
    n != 0 && (n & (n - 1)) == 0
}

pub fn normalize_sample(sample: f32, target_peak: f32) -> f32 {
    let abs_sample = sample.abs();
    if abs_sample > target_peak {
        // Soft limiting to prevent harsh clipping
        let ratio = target_peak / abs_sample;
        let soft_ratio = 1.0 - expf(-3.0 * ratio);
        sample * soft_ratio
    } else {
        sample
    }
}

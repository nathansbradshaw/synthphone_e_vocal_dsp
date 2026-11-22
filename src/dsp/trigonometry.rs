//! Shared bounded-phase trigonometry for the real-time ARM audio paths.
use core::f32::consts::TAU;

// A quarter-wave table gives 1024 samples per turn using 1028 bytes of flash.
// Linear interpolation has <5e-6 absolute error; unlike libm calls its cost is
// bounded and small enough for independent voices at every FFT hop on M7.
const fn quarter_wave() -> [f32; 257] {
    let mut table = [0.0; 257];
    let mut i = 0;
    while i <= 256 {
        let x = i as f64 * core::f64::consts::FRAC_PI_2 / 256.0;
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
static QUARTER: [f32; 257] = quarter_wave();

#[inline(always)]
fn table_sin(index: usize) -> f32 {
    let index = index & 1023;
    let offset = index & 255;
    match index >> 8 {
        0 => QUARTER[offset],
        1 => QUARTER[256 - offset],
        2 => -QUARTER[offset],
        _ => -QUARTER[256 - offset],
    }
}

/// Input is the sum of two phases in [-pi,pi], hence in [-tau,tau].
#[inline(always)]
pub(crate) fn sin_cos(phase: f32) -> (f32, f32) {
    let positive = if phase < 0.0 { phase + TAU } else { phase };
    let position = positive * (1024.0 / TAU);
    let index = position as usize;
    let fraction = position - index as f32;
    let s = table_sin(index);
    let c = table_sin(index + 256);
    (
        s + fraction * (table_sin(index + 1) - s),
        c + fraction * (table_sin(index + 257) - c),
    )
}

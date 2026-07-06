# Performance Recommendations (DSP crate)

Findings from a hot-path review on 2026-07-06. Companion to the firmware repo's
`docs/performance-recommendations.md` (build flags, task/lock structure) — **most of the
CPU burned per second lives in this crate**, so these items matter more than any build
flag.

Target context: Daisy Seed STM32H750 (Cortex-M7 @ 480 MHz, FPv5 single-precision FPU).
The firmware runs `process_vocal_effects_1024` once per hop: 256 samples @ 48 kHz →
**187.5 hops/s, budget 5.33 ms (≈2.56 M cycles) per hop**.

Why libm is the enemy here: `libm` is a *software* float library. Even with the FPU
enabled, `libm::sqrtf` never becomes the 14-cycle `VSQRT` instruction, and `atan2f` /
`sinf` / `fmodf` / `expf` each cost roughly 80–200 cycles of integer bit-twiddling. The
per-bin loops below call them ~1,500–3,000 times per hop.

Ordered by expected payoff. Every item preserves the public API unless noted.

---

## P1. Fast-math module for the per-bin hot loops (biggest win)

**Estimated saving: 0.5–0.9 ms per hop (10–17 % of the hop budget).**

Create `src/dsp/fast_math.rs`, export it from `src/dsp/mod.rs`, and switch the call
sites listed below. Keep `libm` for everything *outside* the per-bin loops (setup code,
tests, YIN — YIN only does adds/multiplies in its inner loop and is already fine).

### P1a. `fast_sqrt` — hardware VSQRT

Used for per-bin magnitudes: `frequency_analysis.rs:335` (phase-vocoder analysis, 512
calls/hop) and `effects/mod.rs:200-206` (vocode, 1024 calls/hop — see also P3).

```rust
/// Hardware square root (VSQRT, 14 cycles) on Cortex-M with an FPU.
/// Exact — same result as libm::sqrtf for normal inputs.
#[cfg(feature = "cortex-m-fpu")]
#[inline(always)]
pub fn fast_sqrt(x: f32) -> f32 {
    let y: f32;
    unsafe {
        core::arch::asm!(
            "vsqrt.f32 {o}, {i}",
            o = out(sreg) y,
            i = in(sreg) x,
            options(pure, nomem, nostack),
        );
    }
    y
}

#[cfg(not(feature = "cortex-m-fpu"))]
#[inline(always)]
pub fn fast_sqrt(x: f32) -> f32 {
    libm::sqrtf(x)
}
```

Add to `Cargo.toml` `[features]`: `cortex-m-fpu = []`. The firmware enables it in its
own `Cargo.toml` dependency line. Host tests keep the libm fallback, so results are
identical on both paths (VSQRT is IEEE-exact).

After implementing, verify the instruction is really emitted:
`cargo objdump --release -- -d | grep vsqrt` (in the firmware repo).

### P1b. `fast_atan2` — polynomial approximation

Used at `frequency_analysis.rs:336` (512 calls/hop).

```rust
/// atan2 approximation, max error ≈ 0.005 rad.
/// For the phase vocoder this is a frequency error < 0.004 bins (< 0.2 Hz at
/// N=1024 / 48 kHz) — far below the pitch detector's own resolution.
#[inline(always)]
pub fn fast_atan2(y: f32, x: f32) -> f32 {
    use core::f32::consts::FRAC_PI_4;
    if x == 0.0 && y == 0.0 {
        return 0.0;
    }
    let abs_y = y.abs() + 1e-20; // avoid 0/0
    let (r, base) = if x >= 0.0 {
        ((x - abs_y) / (x + abs_y), FRAC_PI_4)
    } else {
        ((x + abs_y) / (abs_y - x), 3.0 * FRAC_PI_4)
    };
    let angle = base + (0.1963 * r * r - 0.9817) * r;
    if y < 0.0 { -angle } else { angle }
}
```

### P1c. `wrap_pm_pi` — replace `wrap_phase`'s `fmodf`

`wrap_phase` (`frequency_analysis.rs:317-323`) calls `libm::fmodf` — one of the most
expensive libm routines — 1,024 times per hop (once per bin in analysis at line 342 and
synthesis at line 369). Replace the *body* of `wrap_phase` (keep the name and signature
so call sites don't change):

```rust
/// Round to nearest integer without libm: the "magic number" trick.
/// Valid for |x| < 2^22, which covers every phase value in this crate.
#[inline(always)]
fn round_nearest(x: f32) -> f32 {
    const MAGIC: f32 = 12_582_912.0; // 1.5 * 2^23
    (x + MAGIC) - MAGIC
}

#[inline(always)]
pub fn wrap_phase(phase_in: f32) -> f32 {
    const TWO_PI: f32 = 2.0 * PI;
    const INV_TWO_PI: f32 = 1.0 / TWO_PI;
    phase_in - TWO_PI * round_nearest(phase_in * INV_TWO_PI)
}
```

Behavioral note: the result lands in [−π−ε, π+ε] instead of exactly (−π, π]. Every
consumer (bin-deviation math, `fast_sin`/`fast_cos` below) tolerates this. Do NOT
"simplify" `round_nearest` to `x as i32 as f32` — that truncates toward zero and breaks
negative phases.

### P1d. `fast_sin` / `fast_cos` — replace per-bin `sinf`/`cosf` in synthesis

`perform_phase_vocoder_synthesis` (`frequency_analysis.rs:372-375`) calls
`libm::cosf` + `libm::sinf` per bin (1,024 calls/hop). The input `out_phase` is already
wrapped to ≈[−π, π], which is exactly what this approximation needs:

```rust
/// Parabolic sine approximation with one refinement step.
/// REQUIRES input in [-π-0.01, π+0.01]. Max error ≈ 1e-3 (≈ -60 dBFS),
/// inaudible under a vocal signal.
#[inline(always)]
pub fn fast_sin(x: f32) -> f32 {
    use core::f32::consts::PI;
    const B: f32 = 4.0 / PI;
    const C: f32 = -4.0 / (PI * PI);
    let y = B * x + C * x * x.abs();
    0.225 * (y * y.abs() - y) + y
}

#[inline(always)]
pub fn fast_cos(x: f32) -> f32 {
    use core::f32::consts::{FRAC_PI_2, PI};
    let mut x = x + FRAC_PI_2;
    if x > PI {
        x -= 2.0 * PI;
    }
    fast_sin(x)
}
```

While editing that loop, also **skip the trig for silent bins** — after spectral
shifting most bins carry zero magnitude, but they currently pay full price:

```rust
for i in 0..N / 2 {
    let amplitude = synthesis_magnitudes[i];
    // ... compute phase_diff and out_phase exactly as today (cheap now) ...
    let out_phase = wrap_phase(last_output_phases[i] + phase_diff);
    last_output_phases[i] = out_phase; // ALWAYS update — phase continuity across hops

    if amplitude == 0.0 {
        full_spectrum[i] = microfft::Complex32 { re: 0.0, im: 0.0 };
    } else {
        full_spectrum[i] = microfft::Complex32 {
            re: amplitude * fast_cos(out_phase),
            im: amplitude * fast_sin(out_phase),
        };
    }
    // conjugate-symmetry mirror unchanged
}
```

### P1e. `floorf` → cast for non-negative values

For `x >= 0.0`, `x as usize` truncates — identical to `floorf` — and compiles to a
single `VCVT` instruction instead of a libm call. The harmony loops do this up to
8 voices × 512 bins = 4,096 times per hop. Exact sites (all operate on provably
non-negative values):

| File:line | Current | Replace with |
|---|---|---|
| `effects/mod.rs:131` | `(floorf(new_bin_f + 0.5) * octave_factor) as usize` | `((new_bin_f + 0.5) as usize as f32 * octave_factor) as usize` — careful: floor first, *then* scale, to preserve current behavior |
| `effects/mod.rs:324` | `(floorf(i as f32 * pitch_shift_ratio + 0.5) * octave_factor) as usize` | same pattern as above |
| `effects/mod.rs:468` | `floorf(new_bin_f) as usize` | `new_bin_f as usize` |
| `effects/mod.rs:656` | `floorf(new_bin_f) as usize` | `new_bin_f as usize` |

For lines 131/324 the exact replacement is:

```rust
let rounded = (new_bin_f + 0.5) as usize; // == floorf(new_bin_f + 0.5) for x >= 0
let new_bin = (rounded as f32 * octave_factor) as usize;
```

### P1f. Freebie: route the soft-clip `expf` through `fast_exp`

The limiter at `effects/mod.rs:161, 519, 700` calls `libm::expf` only when a sample
exceeds 0.95, so it's not hot — but once P2's `fast_exp` exists, use it here too and
deduplicate the three copies into one `fn soft_clip(sample: f32) -> f32` in
`fast_math.rs` (this is also v0.2-recommendations §8b).

### P1 validation

Add a `#[cfg(test)]` module to `fast_math.rs` asserting each approximation against
libm over dense sweeps:

```rust
// fast_sqrt: exact (or libm fallback) — assert bit-equality on host is NOT required;
//            assert |fast - libm| / libm < 1e-6 over [1e-12, 1e6].
// fast_atan2: |err| < 0.006 rad over a 1000×1000 grid of (y, x) in [-10, 10]².
// wrap_phase: |fast_result - reference| < 1e-3, reference = old fmodf implementation
//             (keep a private copy named wrap_phase_exact inside the test module),
//             sweep [-1000.0, 1000.0] in steps of 0.001.
// fast_sin/fast_cos: |err| < 2e-3 over [-3.2, 3.2].
```

Then run the full existing test suite. Tests that assert exact sample values may need
tolerance loosening to ~1e-2 — that is expected and fine; anything failing by *more*
than that indicates a real mistake.

---

## P2. The formant envelope costs 2 extra FFTs + 1,024 log/exp per hop

**Estimated saving: 0.3–0.6 ms per hop in PitchControl and Dry modes.**

`extract_cepstral_envelope` (`signal_processing.rs:6-48`) runs **every hop** in
PitchControl and Dry (`effects/mod.rs:94, 302`) and internally performs one inverse FFT
+ one forward FFT + 512 `logf` (`signal_processing.rs:19`) + 512 `expf`
(`signal_processing.rs:46`). That makes those modes 4-FFT-per-hop pipelines where only
2 FFTs process actual audio. (Harmony already avoids this with a cached
moving-average envelope refreshed every 16 hops.)

Two independent steps:

### P2a. Replace `logf`/`expf` with fast approximations (no quality risk)

Add to `fast_math.rs` (these are the well-known "fastapprox" formulas, max relative
error ≈ 2e-4 — far below what an envelope needs):

```rust
#[inline(always)]
pub fn fast_log2(x: f32) -> f32 {
    let vx = x.to_bits();
    let mx = f32::from_bits((vx & 0x007F_FFFF) | 0x3F00_0000);
    let y = vx as f32 * 1.192_092_9e-7;
    y - 124.225_52 - 1.498_030_3 * mx - 1.725_88 / (0.352_088_72 + mx)
}

#[inline(always)]
pub fn fast_ln(x: f32) -> f32 {
    core::f32::consts::LN_2 * fast_log2(x)
}

#[inline(always)]
pub fn fast_pow2(p: f32) -> f32 {
    let offset: f32 = if p < 0.0 { 1.0 } else { 0.0 };
    let clipp = if p < -126.0 { -126.0 } else { p };
    let w = clipp as i32;
    let z = clipp - w as f32 + offset;
    let v = ((1u64 << 23) as f32
        * (clipp + 121.274_06 + 27.728_024 / (4.842_525_5 - z) - 1.490_129_1 * z))
        as u32;
    f32::from_bits(v)
}

#[inline(always)]
pub fn fast_exp(x: f32) -> f32 {
    fast_pow2(x * core::f32::consts::LOG2_E)
}
```

Callers already clamp the `fast_ln` input with `.max(1e-6)`, so no domain issues.
Validation: assert relative error vs `libm::logf`/`libm::expf` < 1e-3 over
[1e-6, 100.0] and [-30.0, 5.0] respectively.

### P2b. Offer the cheap envelope as a config choice (saves the 2 FFTs)

`extract_simple_envelope` (`signal_processing.rs:50-62`) already exists — a moving
average, no FFTs — but nothing calls it. Add a config field (this becomes
`EnvelopeMethod { Cepstral, Smoothed { width_hz } }` in the v0.2 API spec §2, so name it
compatibly):

```rust
// in VocalEffectsConfig
pub use_cepstral_envelope: bool, // default true = current behavior
```

In `process_pitch_correction_generic` (`effects/mod.rs:94`) and `process_dry_generic`
(`effects/mod.rs:302`), branch on it:

```rust
if config.use_cepstral_envelope {
    extract_cepstral_envelope::<N, HALF_N, F>(&analysis_magnitudes, cached_envelope);
} else {
    extract_simple_envelope::<HALF_N>(&analysis_magnitudes, cached_envelope);
}
```

Then A/B on hardware (toggle the flag in the firmware, listen to PitchControl on a
sung vowel-to-consonant transition). If Smoothed sounds acceptable, the firmware ships
with it and drops ~2 FFTs/hop; if not, P2a alone still stands. Do **not** cadence-cache
the envelope in these modes — the code comments at `effects/mod.rs:87-93` explain why a
stale envelope is audible here.

---

## P3. Vocode: one sqrt per bin instead of two

`effects/mod.rs:198-213` computes `sqrtf(mod)` and `sqrtf(car)` then divides. Since
`mod_mag / car_mag == sqrt(mod_energy / car_energy)`, do:

```rust
let mod_energy =
    modulator_fft[i].re * modulator_fft[i].re + modulator_fft[i].im * modulator_fft[i].im;
let car_energy =
    carrier_fft[i].re * carrier_fft[i].re + carrier_fft[i].im * carrier_fft[i].im;

// Gate on energy: 1e-8 == (previous magnitude gate 1e-4)²
let scale_factor = if car_energy > 1e-8 {
    fast_sqrt(mod_energy / car_energy)
} else {
    0.0
};
```

Halves the sqrt count (512 → saved), removes one multiply chain. With P1a's `fast_sqrt`
this bin loop becomes almost free.

---

## P4. Oscillator: `libm::sinf` per sample → sine lookup table

`Oscillator::next_value` (`src/audio/oscillator.rs:37`) calls `libm::sinf` per sample.
This runs in **two hot places in the firmware**: the audio interrupt's synth voices
(up to 8 voices × 48 kHz = 384k calls/s ≈ 8–10 % of the whole CPU when chords are held)
and the vocoder carrier generation (8 osc × 256 samples per hop).

Replace the `Waveform::Sine` arm with a 256-entry interpolated LUT. The table must be
buildable in a `const` context (no_std, no allocator, and `libm` calls aren't const) —
use a Taylor series, which is accurate to ~5e-4 for |x| ≤ π:

```rust
// in oscillator.rs (or fast_math.rs)

/// sin(x) for x in [-π, π], const-evaluable. Taylor to x^11, max err ≈ 5e-4.
const fn taylor_sin(x: f32) -> f32 {
    let x2 = x * x;
    let x3 = x2 * x;
    let x5 = x3 * x2;
    let x7 = x5 * x2;
    let x9 = x7 * x2;
    let x11 = x9 * x2;
    x - x3 / 6.0 + x5 / 120.0 - x7 / 5040.0 + x9 / 362880.0 - x11 / 39916800.0
}

/// 257 entries so `idx + 1` never wraps during interpolation.
/// SINE_LUT[i] == sin(2π · i / 256)
static SINE_LUT: [f32; 257] = {
    let mut t = [0.0f32; 257];
    let mut i = 0;
    while i < 257 {
        // map i/256 ∈ [0,1] → x ∈ [-π, π]; sin(2πt) == -sin(2πt - π)
        let x = 2.0 * core::f32::consts::PI * (i as f32 / 256.0) - core::f32::consts::PI;
        t[i] = -taylor_sin(x);
        i += 1;
    }
    t
};

#[inline(always)]
fn lut_sine(phase: f32) -> f32 {
    // phase in [0, 1)
    let pos = phase * 256.0;
    let idx = pos as usize; // 0..=255
    let frac = pos - idx as f32;
    SINE_LUT[idx] + (SINE_LUT[idx + 1] - SINE_LUT[idx]) * frac
}
```

Then in `next_value`: `Waveform::Sine => lut_sine(self.phase)` (note: current code
computes `sinf(2π·phase)` — the LUT already bakes in the 2π, pass `self.phase`
directly). Total error (Taylor + linear interp) ≈ 6e-4 ≈ −64 dB: inaudible.

Validation: test `(lut_sine(t) - libm::sinf(2.0 * PI * t)).abs() < 1e-3` for
`t in (0..10_000).map(|i| i as f32 / 10_000.0)`.

---

## P5. `bitcrush` and `sample_rate_reduce` run full price when set to "off"

These run **per sample in the firmware's audio interrupt** (48 kHz) regardless of
settings. `bitcrush` (`frequency_analysis.rs:296-304`) calls `libm::roundf` even at
`bit_depth = 32`, where quantization is a mathematical no-op. `sample_rate_reduce`
(`frequency_analysis.rs:274-293`) does modulo bookkeeping even at `factor = 1`.

```rust
pub fn bitcrush(sample: f32, bit_depth: i8) -> f32 {
    // At >= 24 bits the step size is below f32 mantissa precision — identity.
    if bit_depth >= 24 {
        return sample;
    }
    // ... existing body unchanged ...
}

pub fn sample_rate_reduce(
    sample: f32,
    factor: i8,
    hold_counter: &mut i32,
    held_value: &mut f32,
) -> f32 {
    // factor <= 1 means "no reduction" — pass through and keep state coherent.
    // This also fixes the divide-by-zero TODO for factor == 0.
    if factor <= 1 {
        *hold_counter = 0;
        *held_value = sample;
        return sample;
    }
    // ... existing body unchanged (the `if factor != 0` guard can then go) ...
}
```

This is the *default* state of both effects, so the default path drops two libm-adjacent
calls per sample.

---

## P6. RingBuffer: add slice operations (needed by firmware lock-hoisting)

The firmware's audio interrupt currently does per-sample `push`/`pop`, each a pair of
atomic ops, *inside an RTIC lock taken per sample*. The firmware-side fix (its doc,
item 2) needs block transfer methods here. Add to `src/ring_buffer.rs`, preserving the
existing semantics exactly (producer overwrites, consumer zeroes the slot after read —
the zeroing is load-bearing for the overlap-add output path):

```rust
/// Producer side: push a block with one atomic store.
pub fn push_slice(&self, samples: &[f32]) {
    let mut w = self.write.load(Ordering::Relaxed);
    let buf = unsafe { &mut *self.buf.get() };
    for &v in samples {
        buf[w as usize & (N - 1)] = v;
        w = w.wrapping_add(1);
    }
    self.write.store(w, Ordering::Release);
}

/// Consumer side: pop a block with one atomic store. Clears slots after
/// reading (required by write_overlapped_samples' accumulation).
pub fn pop_slice(&self, out: &mut [f32]) {
    let mut r = self.read.load(Ordering::Relaxed);
    let buf = unsafe { &mut *self.buf.get() };
    for o in out.iter_mut() {
        let cell = &mut buf[r as usize & (N - 1)];
        *o = *cell;
        *cell = 0.0;
        r = r.wrapping_add(1);
    }
    self.read.store(r, Ordering::Release);
}
```

Add unit tests mirroring the existing push/pop tests (push_slice then pop_slice
round-trips; pop_slice zeroes; interleaving with single push/pop stays consistent).

---

## P7. Return-by-value and oversized scratch arrays (rides with the v0.2 API break)

Each `process_*_generic` returns `[f32; N]` by value (a 4 KB copy per hop, twice after
the dispatch wrapper), zero-fills `[Complex32; N]` (8 KB) plus `synthesis_magnitudes` /
`synthesis_frequencies` sized `[f32; N]` when only `HALF_N` elements are ever used
(`effects/mod.rs:58-59, 259-260, 387-388, 556-557`). Total: ~30–40 KB of stack traffic
and ~10 KB of memset per hop in the highest-priority-but-one task, all in DTCM.

Don't patch this piecemeal — it is exactly what api-spec.md §1 fixes
(`VocalProcessor` owns state, `process_frame` writes into `&mut output`). When
implementing v0.2 §1, also:

- size `synthesis_magnitudes` / `synthesis_frequencies` as `[f32; HALF_N]` (change
  `perform_phase_vocoder_synthesis` to take `&[f32]` slices of length `N/2`);
- make the scratch buffers (`full_spectrum`, cepstrum buffers) fields of
  `VocalProcessor` so they're zero-filled only where the algorithm requires, not
  re-allocated on the stack per hop.

---

## Measuring on target

Wrap the hop in DWT cycle counts before/after each item (RTT-print every ~100 hops):

```rust
// once in init(): core.DCB.enable_trace(); core.DWT.enable_cycle_counter();
let t0 = cortex_m::peripheral::DWT::cycle_count();
// ... process_vocal_effects_1024 ...
let cycles = cortex_m::peripheral::DWT::cycle_count().wrapping_sub(t0);
// budget: 2.56 M cycles per hop; aim to keep the FFT task under ~50 % of it.
```

## Workflow note for the firmware

The firmware pins this crate by git commit
(`synthphone-e-vocal-dsp = { git = ..., commit = "2d33801..." }`). While iterating,
switch to the commented-out path dependency
(`synthphone-e-vocal-dsp = { path = "../synthphone_e_vocal_dsp/" }`) in the firmware's
`Cargo.toml`, and bump the pinned commit once the crate changes are pushed. Also note
the DSP working tree has uncommitted changes — commit those first so measurements have
a stable baseline.

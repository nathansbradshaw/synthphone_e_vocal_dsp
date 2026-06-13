# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Commands

```bash
# Build
cargo build
cargo build --release

# Test
cargo test --all-features
cargo test --no-default-features --features embedded
cargo test --doc

# Lint and format
cargo fmt
cargo fmt --check
cargo clippy --all-features -- -D warnings

# Benchmarks
cargo bench

# Embedded cross-compilation targets
rustup target add thumbv7em-none-eabihf
rustup target add thumbv6m-none-eabi
cargo build --target thumbv7em-none-eabihf --no-default-features --features embedded
```

## Architecture

This is a `no_std` Rust DSP library for real-time vocal effects on embedded and desktop platforms. All audio processing paths must be allocation-free with bounded execution time.

### Processing pipeline

The main entry points are the four public functions in `src/vocal_effects.rs`:
`process_vocal_effects_512/1024/2048/4096`. Each monomorphizes the generic
`process_vocal_effects<const N, const HALF_N, F>` over an FFT size and the
corresponding `FftOps` implementor (`Fft512`, `Fft1024`, `Fft2048`, `Fft4096` in
`src/dsp/fft.rs`).

Inside the generic function, dispatch on `ProcessingMode` routes to one of four
effect implementations in `src/effects/mod.rs`:
- `PitchControl` — phase vocoder autotune, uses HPS for fundamental detection
- `Vocode` — vocal formant transfer to a carrier signal
- `Dry` — pitch/formant shifting without correction
- `Harmony` — pitch-shifts to MIDI-provided target frequencies and mixes voices

### Key types

| Type | File | Role |
|------|------|------|
| `VocalEffectsConfig` | `src/config.rs` | FFT size, hop ratio, sample rate, HPS params |
| `MusicalSettings` | `src/state.rs` | Key, note, octave, formant, mode, MIDI freqs |
| `ProcessingMode` | `src/state.rs` | Enum selecting the effect algorithm |
| `FftOps<N, HALF_N>` | `src/dsp/fft.rs` | Trait abstracting `microfft` calls per size |

### DSP modules

- `src/dsp/frequency_analysis.rs` — phase vocoder analysis/synthesis (`perform_phase_vocoder_analysis`, `perform_phase_vocoder_synthesis`) and HPS fundamental detection (`find_fundamental_frequency`)
- `src/dsp/signal_processing.rs` — cepstral envelope extraction (`extract_cepstral_envelope`) and pitch shift ratio calculation (`calculate_pitch_shift`)
- `src/dsp/windowing.rs` — static Hann window tables for each FFT size
- `src/audio/keys.rs` — musical scale lookup (`get_scale_by_key`, `get_frequency`)
- `src/audio/frequencies.rs` — nearest-note snapping (`find_nearest_note_in_key`)
- `src/audio/oscillator.rs` — synthesis oscillator
- `src/math.rs` — `no_std`-safe math helpers backed by `libm`
- `src/ring_buffer.rs` — lock-free static ring buffer

### Feature flags

| Flag | Effect |
|------|--------|
| `embedded` (default) | Enables embedded-specific code paths |
| `std` | Enables `critical-section/std`, unlocks `std`-dependent features |
| `cortex-m` | Pulls in `cortex-m` crate for ARM targets |
| `formant-shifting` | Enables cepstral formant processing (implies `cepstral-smoothing`) |
| `debug-logging` | Enables `log` crate output |

### Constraints

- No heap allocations anywhere in processing paths — all buffers are fixed-size arrays on the stack or `static`
- `libm` is used for all math (`logf`, `expf`, `sqrtf`, `fabsf`, `floorf`) instead of `std::f32`
- `microfft` provides in-place fixed-size FFT; the `FftOps` trait is the only abstraction layer over it
- Soft-clipping (`|sample| > 0.95`) is applied on output in `PitchControl` and `Harmony` modes

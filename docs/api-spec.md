# v0.2.0 API Specification

Companion to [v0.2-recommendations.md](./v0.2-recommendations.md). This is the concrete
public surface proposed for 0.2.0 — signatures are normative. (0.2.0 is deliberately
pre-1.0: the API must survive the Synthphone firmware migration and real desktop use
before we promise stability.) The crate stays
`synthphone-e-vocal-dsp` (decided 2026-07: the Synthphone is the primary use case;
platform-agnosticism is a property of the crate, not its name).

## Design principles

1. **`no_std` + zero-allocation core.** Every type is `const`-constructible or plain
   data; the caller decides placement (RTIC local, `static`, `Box`, stack).
2. **The processor owns its state.** No phase arrays, envelope caches, or pitch ratios
   threaded through call sites. One struct = one voice-processing instance.
3. **Two altitudes, same engine.** A *frame API* (`VocalProcessor`) for callers that
   already have STFT-aligned frames (RTIC split-task designs), and a *streaming API*
   (`StreamingProcessor`) that owns the hop/overlap-add plumbing for callers that just
   have an audio callback (CPAL, plugins, WASM).
4. **Additive features only.** `no_std` by default; features only ever add capability.
5. **Invalid states unrepresentable.** Enums instead of magic ints; validated configs
   tied to const generics at construction.
6. **The Daisy Seed budget is the performance ruler.** When a knob and the hot path
   conflict, the hot path wins: every knob is read at most once per hop, per-sample
   inner loops stay branch-free, and a knob that would cost per-sample work must
   instead select between precompiled paths (see `EnvelopeMethod`). Desktop gets its
   flexibility from *more* knob positions, never from a slower default.
7. **Reject crashes, not weird sounds.** Builders return `Err` only for values that
   cause UB, NaN propagation, or broken STFT reconstruction (e.g. hop ratio outside
   1/16..=1/2). A magnitude threshold that gates the whole spectrum, a limiter
   threshold above 1.0, an 8× formant warp — all legal. "Sounds wrong" is a feature
   for sound design, not an error.

## Module map

```text
synthphone_e_vocal_dsp
├── VocalProcessor<const N>      // frame API (root re-export)
├── StreamingProcessor<const N>  // streaming API (root re-export)
├── config                       // ProcessorConfig + builder
├── settings                     // MusicalSettings, ProcessingMode, Key, FormantMode
├── error                        // Error enum
├── blocks                       // composable spectral primitives (extension API)
├── tap                          // per-hop introspection for GUIs (feature "tap")
├── pitch                        // YIN + HPS detection, note quantization
├── music                        // keys, scales, note↔frequency tables
├── fx                           // bitcrush, sample-rate reduce, normalize
├── osc                          // Oscillator, Waveform
├── spsc                         // ring buffer (Producer/Consumer)
└── raw                          // low-level free functions (semver-exempt, doc(hidden))
```

---

## 1. Settings types (`settings`)

```rust
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ProcessingMode {
    #[default]
    PitchCorrect,   // was: PitchControl
    Vocode,
    PitchShift,     // was: Dry (shift + formant preservation, no correction)
    Harmony,
    Bypass,         // analysis still runs; output == input (new)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    Major(Note),        // Key::Major(Note::C) replaces key = 0
    Minor(Note),
    Chromatic,          // quantize to any semitone (new; today: nearest-note path)
    Custom(ScaleMask),  // arbitrary scales: pentatonic, dorian, one-note hard-tune…
}

/// 12-bit semitone mask relative to a root — any scale is expressible.
/// `ScaleMask::new(Note::C, &[0, 2, 4, 7, 9])` = C major pentatonic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScaleMask { root: Note, bits: u16 }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Note { C, Db, D, Eb, E, F, Gb, G, Ab, A, Bb, B }

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum FormantMode {
    #[default]
    Preserve,               // was: formant = 0
    Shift(f32),             // ratio; <1.0 down ("male"), >1.0 up ("female")
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub struct MusicalSettings<'a> {
    pub mode: ProcessingMode,
    pub key: Key,
    /// Some(semitone 1..=12 within key) forces a scale degree; None = auto-quantize.
    pub forced_note: Option<u8>,
    /// Continuous transpose in semitones: 12.0 = up an octave, -12.0 = down,
    /// fractional values = detune (voice changers live here). Replaces the
    /// firmware's 1/2/4 octave encoding — the Synthphone passes ±12.0 / 0.0.
    /// Cost: one exp2 per hop, nothing per sample.
    pub pitch_shift_semitones: f32,
    pub formant: FormantMode,
    /// Wet/dry blend applied post-synthesis: 0.0 = input passthrough, 1.0 = full
    /// effect. One multiply-add per sample. (New — today Harmony hardcodes its
    /// dry+wet sum and PitchShift hardcodes 0.96/0.04.)
    pub mix: f32,
    /// Target frequencies (Hz) driving Vocode carriers / Harmony voices.
    /// Zero entries are ignored. Borrowed, so voice count is the caller's choice
    /// (was: [f32; 8] hardcoded). Per-voice gains/voicing beyond equal-weight:
    /// use `blocks::add_shifted` (§6).
    pub target_frequencies: &'a [f32],
}
```

**Decided (2026-07): borrowed slice with a lifetime.** Both candidate designs are
heap-free, but the borrow adds no copies and no arbitrary voice ceiling. Settings are
plain `Copy` data rebuilt per hop from app state, so nothing stores them long enough
for the lifetime to bite; `MusicalSettings<'static>` (e.g. with an empty or const
slice) remains available where a `'static` value is genuinely needed.

`MusicalSettings::default()` = PitchCorrect, `Key::Chromatic`, no forced note, zero
transpose, `FormantMode::Preserve`, `mix: 1.0`, empty targets (`&[]`, so `Default`
yields `MusicalSettings<'static>`).

## 2. Configuration (`config`)

Construction-validated, fields private, tied to the processor's `N` at build time.

```rust
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ProcessorConfig {
    // all fields private
}

impl ProcessorConfig {
    /// Defaults: 48 kHz, hop = N/4, correction strength 0.999, transition 0.1,
    /// range 50–4000 Hz. `N` is fixed by the processor, not stored here.
    pub const fn new(sample_rate_hz: f32) -> Self;

    // Builder-style, all validating, all const where possible:
    pub fn hop_ratio(self, ratio: f32) -> Result<Self, Error>;          // 1/16..=1/2
    pub fn correction_strength(self, s: f32) -> Result<Self, Error>;    // 0.0..=1.0
    pub fn transition_speed(self, s: f32) -> Result<Self, Error>;       // 0.0..=1.0
    pub fn frequency_range(self, min_hz: f32, max_hz: f32) -> Result<Self, Error>;
    pub fn magnitude_threshold(self, t: f32) -> Result<Self, Error>;

    // Promoted from hardcoded values (v0.2-recommendations.md §8b):
    pub fn limiter_threshold(self, t: f32) -> Result<Self, Error>;       // was 0.95, ×3 copies
    pub fn harmony_envelope_refresh_hops(self, n: u8) -> Result<Self, Error>; // was 16
    pub fn reference_pitch_hz(self, a4: f32) -> Result<Self, Error>;     // was implicit 440.0
    // (the old hardcoded 0.96/0.04 PitchShift blend is subsumed by settings.mix)

    /// THE CPU lever. Cepstral = two extra N-point FFTs per hop (accurate formants);
    /// Smoothed = a moving average (cheap, coarser). Daisy Harmony mode already runs
    /// Smoothed today; desktop defaults to Cepstral everywhere.
    pub fn envelope_method(self, m: EnvelopeMethod) -> Self;

    /// Pitch-detection knobs, all with vocal-tuned defaults:
    /// YIN vocal range (was hardcoded 80–600 Hz), YIN aperiodicity threshold
    /// (was 0.15), YIN decimation factor (was 8 — trade accuracy vs. inner-loop
    /// cost), HPS noise floor + downsample factors (already config today).
    pub fn pitch_detector(self, cfg: PitchDetectorConfig) -> Self;

    // Getters (bin_width/spectrum/hop need N, so they live on the processor too):
    pub fn sample_rate(&self) -> f32;
    pub fn hop_size(&self, fft_size: usize) -> usize;
}

#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EnvelopeMethod {
    /// Cepstral liftering — accurate formants, +2 N-point FFTs per hop.
    Cepstral,
    /// Moving-average smoothing — cheap, coarser. Width in Hz (converted to bins
    /// internally, fixing today's bins-vs-Hz confusion at different N).
    Smoothed { width_hz: f32 },
}
```

HPS tuning knobs (`hps_noise_floor_ratio`, downsample factors) move to the nested
`PitchDetectorConfig` — they configure detection, not effects, and most users never
touch them; the ones who do (sound designers, non-vocal material) get every dial.

Validation follows design principle 7: `Err` only guards crash/NaN/reconstruction
correctness. Extreme-but-finite values pass — `magnitude_threshold(0.5)` gating almost
the whole spectrum into silence-with-artifacts is a legitimate creative setting, not a
mistake to protect anyone from.

## 3. Error type (`error`)

```rust
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    /// Config parameter outside documented range (carries which one).
    InvalidParameter(Parameter),
}

#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Parameter { HopRatio, CorrectionStrength, TransitionSpeed, FrequencyRange, MagnitudeThreshold }

impl core::fmt::Display for Error { ... }          // no_std-friendly
#[cfg(feature = "std")]
impl std::error::Error for Error {}
```

Drops `BufferSizeMismatch` / `UnsupportedFftSize` / `ProcessingFailed` — buffer sizes
are const-generic (can't mismatch), FFT size is a type parameter (can't be
unsupported), and processing is infallible. There is deliberately no `NotReady`-style
error either: real-time paths never fail, they emit silence (see §13 contracts).

## 4. Frame API — `VocalProcessor<const N: usize>`

The front door. Owns all inter-hop state (~`4.5 × N × 4` bytes; table below).
`N ∈ {512, 1024, 2048, 4096}` enforced by a sealed trait bound.

```rust
pub struct VocalProcessor<const N: usize>
where
    FftSize<N>: SupportedFftSize,   // sealed
{ /* phases, envelope caches, prev pitch ratio, config */ }

impl<const N: usize> VocalProcessor<N> {
    /// Const-constructible → usable in a `static` or RTIC local without unsafe.
    pub const fn new(config: ProcessorConfig) -> Self;

    /// Process one analysis frame (N input samples, hop-aligned, most recent last).
    /// `carrier`: required by Vocode, optional elsewhere. Output is windowed
    /// synthesis, ready for overlap-add at the configured hop.
    pub fn process_frame(
        &mut self,
        input: &[f32; N],
        carrier: Option<&[f32; N]>,
        settings: &MusicalSettings,
        output: &mut [f32; N],
    );

    /// Clear phases/envelopes/ratio (e.g. after a transport stop or long silence).
    /// NOT needed on mode switch — the processor tracks the last `ProcessingMode`
    /// and clears mode-specific state internally when it changes, so switching
    /// modes mid-stream is click-safe by contract.
    pub fn reset(&mut self);

    /// Reconfigure for a new sample rate (desktop hosts renegotiate at any time).
    /// Implies `reset()`. All other config is fixed at construction.
    pub fn set_sample_rate(&mut self, hz: f32);

    /// Detected fundamental of the last processed frame, if voiced.
    pub fn detected_pitch_hz(&self) -> Option<f32>;

    /// Algorithmic latency in samples: N (analysis window) — callers add their own
    /// buffering on top.
    pub const fn latency_samples(&self) -> usize;

    pub const fn hop_size(&self) -> usize;
    pub const fn bin_width(&self) -> f32;
    pub fn config(&self) -> &ProcessorConfig;
}
```

Changes vs. today's `process_vocal_effects_*`:

| Today | v0.2 |
|---|---|
| 9 args, caller owns 5 pieces of state | 4 args, processor owns state |
| Returns `[f32; N]` by value (16 KB copy at 4096) | Writes into `&mut` output |
| `previous_pitch_shift_ratio` manually threaded | Internal (bug in the firmware today) |
| `input` is `&mut` and clobbered | `input` is `&`, processor copies to internal scratch |
| 4 named functions + config.fft_size that can disagree | One generic type; size is the type |

### State footprint (f32 = 4 bytes)

| N | phases (2×N) | envelopes (2×N/2) | scratch (N + spectrum) | total ≈ |
|---|---|---|---|---|
| 512 | 4 KB | 2 KB | 6 KB | **12 KB** |
| 1024 | 8 KB | 4 KB | 12 KB | **24 KB** |
| 2048 | 16 KB | 8 KB | 24 KB | **48 KB** |
| 4096 | 32 KB | 16 KB | 48 KB | **96 KB** |

(Documented in rustdoc per size so embedded users can budget DTCM/SRAM placement.)

## 5. Streaming API — `StreamingProcessor<const N: usize, const BUF: usize>`

Owns the ring buffers, hop counter, windowing, and overlap-add. `BUF` is the internal
ring capacity (power of two, ≥ 2 × N; compile-time asserted).

```rust
pub struct StreamingProcessor<const N: usize, const BUF: usize> {
    processor: VocalProcessor<N>,
    /* in ring, out ring (pre-offset for OLA), carrier ring, hop counter */
}

impl<const N: usize, const BUF: usize> StreamingProcessor<N, BUF> {
    pub const fn new(config: ProcessorConfig) -> Self;

    /// Feed any number of input samples. Returns how many hops became ready
    /// (0 almost always; 1+ when the hop boundary was crossed). Processing
    /// happens *inside* this call — callers on a real-time thread should feed
    /// ≤ hop_size samples per call for bounded work.
    pub fn write(&mut self, input: &[f32], settings: &MusicalSettings) -> usize;

    /// Same, with a carrier stream (Vocode).
    pub fn write_with_carrier(
        &mut self, input: &[f32], carrier: &[f32], settings: &MusicalSettings,
    ) -> usize;

    /// Pull processed samples. Yields silence until the first hop completes
    /// (pipeline latency = N + hop samples, matching the firmware's
    /// `with_offset(FFT_SIZE + 2*HOP)` arrangement — exact constant TBD in impl).
    pub fn read(&mut self, output: &mut [f32]);

    /// One-call convenience for equal-size in/out blocks (the CPAL/plugin shape).
    pub fn process_block(
        &mut self, input: &[f32], output: &mut [f32], settings: &MusicalSettings,
    );

    /// In-place variant — most plugin hosts hand you one buffer for both
    /// directions; forcing a caller-side copy there would be a papercut.
    pub fn process_block_in_place(&mut self, buf: &mut [f32], settings: &MusicalSettings);

    pub fn reset(&mut self);
    pub const fn latency_samples(&self) -> usize;
    pub fn inner(&self) -> &VocalProcessor<N>;          // e.g. detected_pitch_hz()
}
```

**Split-context embedded use** (ISR feeds samples, lower-priority task runs the FFT —
the Synthphone/RTIC shape) does *not* use `StreamingProcessor`; it composes
`spsc::RingBuffer` + `VocalProcessor` directly, exactly as the firmware does today.
Document this pattern with an example rather than trying to make one type safely span
two priorities.

## 6. Extension API — composable spectral blocks (`blocks`)

The four `ProcessingMode`s are compositions of a small set of primitives. Exposing
those primitives — curated, not raw internals — lets users build effects the modes
don't cover (robotizer, whisper, spectral freeze, custom harmony voicings, spectral
gates, tuners, visualizers) without forking the crate or waiting for upstream. The
altitude ladder becomes:

```text
L0  StreamingProcessor   — audio blocks in/out, zero DSP knowledge required
L1  VocalProcessor       — hop-aligned frames, built-in modes
L2  blocks::*            — STFT frames + spectral ops (this section)
L3  raw::*               — doc(hidden), semver-exempt
```

**Guard rail:** the built-in modes are reimplemented *on top of* `blocks` (dogfooding).
The public primitive set is exactly what the engine itself needs — that keeps it small,
proves it sufficient, and means the primitives can't rot. Anything the modes don't need
starts life in `raw` until a real consumer justifies promotion.

### 6.1 The frame type

```rust
/// One analysis frame after phase-vocoder analysis: per-bin magnitude and
/// instantaneous frequency (Hz). This is the currency all blocks trade in.
pub struct SpectralFrame<const HALF_N: usize> {
    pub magnitudes: [f32; HALF_N],
    pub frequencies: [f32; HALF_N],
}
```

### 6.2 STFT bookends

The stateful window/FFT/phase-tracking machinery, split from the effects so any
spectral processing can sit between them:

```rust
pub struct StftAnalyzer<const N: usize, const HALF_N: usize> { /* input phases, window */ }
impl StftAnalyzer<N, HALF_N> {
    pub const fn new(config: &ProcessorConfig) -> Self;
    /// window → FFT → phase-vocoder analysis
    pub fn analyze(&mut self, input: &[f32; N], out: &mut SpectralFrame<HALF_N>);
    pub fn reset(&mut self);
}

pub struct StftSynthesizer<const N: usize, const HALF_N: usize> { /* output phases */ }
impl StftSynthesizer<N, HALF_N> {
    pub const fn new(config: &ProcessorConfig) -> Self;
    /// phase-vocoder synthesis → IFFT → window (output ready for overlap-add)
    pub fn synthesize(&mut self, frame: &SpectralFrame<HALF_N>, out: &mut [f32; N]);
    pub fn reset(&mut self);
}
```

(Same `N`/`HALF_N` pairing discipline as `VocalProcessor`, enforced by the sealed
trait; collapses to one parameter if/when `generic_const_exprs` stabilizes.)

### 6.3 Spectral operations (stateless free functions)

```rust
/// Shift all bins by `ratio`, accumulating into `dst` (call repeatedly to layer
/// harmony voices). Linear two-bin energy spreading, as the harmony engine does today.
pub fn add_shifted<const HALF_N: usize>(
    src: &SpectralFrame<HALF_N>, ratio: f32, gain: f32, dst: &mut SpectralFrame<HALF_N>,
);

/// Cepstral (accurate) and moving-average (cheap) envelope extraction.
pub fn envelope_cepstral<const N: usize, const HALF_N: usize>(
    magnitudes: &[f32; HALF_N], out: &mut [f32; HALF_N],
);
pub fn envelope_smoothed<const HALF_N: usize>(
    magnitudes: &[f32; HALF_N], width_hz: f32, bin_width: f32, out: &mut [f32; HALF_N],
);

/// Source-filter split and recombine (formant preservation / shifting).
pub fn remove_envelope<const HALF_N: usize>(frame: &mut SpectralFrame<HALF_N>, env: &[f32; HALF_N]);
pub fn apply_envelope<const HALF_N: usize>(
    frame: &mut SpectralFrame<HALF_N>, env: &[f32; HALF_N], warp_ratio: f32, // 1.0 = no formant shift
);

/// Impose modulator's magnitude envelope on carrier (the vocoder core).
pub fn vocode<const HALF_N: usize>(
    modulator: &SpectralFrame<HALF_N>, carrier: &SpectralFrame<HALF_N>,
    out: &mut SpectralFrame<HALF_N>,
);

/// The exp-knee soft clipper the modes apply post-IFFT (today copy-pasted ×3).
pub fn soft_clip(samples: &mut [f32], threshold: f32);
```

### 6.4 Custom effects through the standard pipeline

For users who want the full pipeline (windowing, phase bookkeeping, overlap-add
readiness) and only custom spectral math in the middle:

```rust
pub struct FrameContext {
    pub detected_pitch_hz: Option<f32>,
    pub bin_width: f32,
    pub hop_size: usize,
    pub sample_rate: f32,
}

pub trait SpectralEffect<const HALF_N: usize> {
    fn process(&mut self, frame: &mut SpectralFrame<HALF_N>, ctx: &FrameContext);
}
// Blanket impl so closures work: impl<F: FnMut(&mut SpectralFrame, &FrameContext)> SpectralEffect for F

impl<const N: usize> VocalProcessor<N> {
    /// Standard pipeline with a user effect in place of the built-in modes.
    pub fn process_frame_with(
        &mut self,
        input: &[f32; N],
        effect: &mut impl SpectralEffect<HALF_N>,
        output: &mut [f32; N],
    );

    /// Analysis without synthesis — tuners, meters, visualizers. Runs pitch
    /// detection and fills `out`; skips the inverse path entirely (~half the cost).
    pub fn analyze_frame(&mut self, input: &[f32; N], out: &mut SpectralFrame<HALF_N>)
        -> FrameContext;
}
```

Fifteen-line examples this unlocks (each a candidate for `examples/`):

- **Robotizer**: in the effect, set every `frequencies[i]` to its bin center — classic
  monotone robot.
- **Whisperizer**: randomize frequencies within each bin — pitchless breath.
- **Spectral freeze**: copy one `SpectralFrame`, return it forever.
- **Custom harmony**: `add_shifted` with user-chosen intervals/gains per voice —
  drop-in replacement for the built-in harmony's fixed MIDI-driven voicing.
- **Guitar tuner**: `analyze_frame` + `pitch::quantize_to_key`, no synthesis.

### 6.5 What stays private

Phase-unwrap internals (`wrap_phase`, raw analysis/synthesis loops), the FFT dispatch
trait (`FftOps` — microfft is an implementation detail we may swap), Hann tables, and
the envelope extrapolation helper. These are the pieces most likely to change when
optimizing; `blocks` is the contract, these are the implementation.

## 7. Diagnostics tap (`tap`, feature-gated)

For hosts that want to *see* the processing: a desktop twin of the Daisy showing
frequency tracks, spectra, formant envelopes, FFT frames, and YIN internals live.
Everything the engine computes per hop becomes observable — without the Daisy ever
paying for it.

### Zero-cost guarantee (design principle 6 applied)

- The entire module is behind the **`tap` feature**. Disabled (the Daisy build):
  the module, the trait, and the tapped methods **do not exist** — not a runtime
  branch, not a null check, zero bytes of flash. `process_frame` is bit-identical
  with and without the feature.
- Enabled, the normal methods are still untouched: tapping is a *separate* entry
  point (`process_frame_tapped`), so even a desktop build only pays when it asks.
- No copies inside the library: the report hands out borrows into live internal
  buffers for the duration of one callback. The observer copies what it wants
  (that's the GUI's memory, not the crate's).
- The only real cost when tapping: a few intermediate buffers that the untapped path
  overwrites in place must persist to end-of-hop (e.g. the pre-shift analysis frame,
  YIN's CMNDF curve ~75 f32). They live inside the tapped call's stack frame, sized
  const, `no_std`-clean — the tap works on embedded too if you want RTT streaming
  to a host plotter.

### The report

One callback per hop, carrying a borrowed view of every pipeline stage:

```rust
#[cfg(feature = "tap")]
pub mod tap {
    /// Everything the engine knows about one hop. All fields are borrows into
    /// processor-internal state, valid only during the callback.
    #[non_exhaustive]
    pub struct HopReport<'a, const N: usize, const HALF_N: usize> {
        // Time domain
        pub input: &'a [f32; N],              // pre-window input frame
        pub output: &'a [f32; N],             // post-synthesis, pre-overlap-add

        // Analysis (pre-effect)
        pub analysis: &'a SpectralFrame<HALF_N>,   // magnitudes + inst. frequencies
        pub envelope: &'a [f32; HALF_N],           // formant envelope (as configured)

        // Synthesis (post-effect)
        pub synthesis: &'a SpectralFrame<HALF_N>,  // what went into the IFFT

        // Pitch detection
        pub detected_pitch_hz: Option<f32>,        // post octave-snap
        pub raw_pitch_hz: Option<f32>,             // pre octave-snap (show the fold!)
        pub target_pitch_hz: Option<f32>,          // where correction is pulling
        pub pitch_shift_ratio: f32,                // smoothed ratio actually applied
        pub yin: Option<YinTrace<'a>>,             // Harmony modes; None elsewhere
        pub hps: Option<HpsTrace<'a>>,             // PitchCorrect; None elsewhere

        // Bookkeeping
        pub mode: ProcessingMode,
        pub hop_index: u64,                        // monotonic since reset
        pub ctx: FrameContext,                     // bin_width, hop, sample_rate
    }

    /// YIN internals — enough to plot the full CMNDF curve with threshold line,
    /// chosen lag, and the parabolic refinement.
    pub struct YinTrace<'a> {
        pub cmndf: &'a [f32],                 // d′[τ] for τ in min_lag..=max_lag
        pub min_lag: usize,
        pub chosen_lag: Option<f32>,          // fractional, post-interpolation
        pub threshold: f32,
        pub effective_sample_rate: f32,       // post-decimation
    }

    /// HPS internals — the product spectrum, the winning bin, and whether the
    /// sub-octave correction fired.
    pub struct HpsTrace<'a> {
        pub product_spectrum: &'a [f32],
        pub peak_bin: usize,
        pub chosen_bin: usize,                // != peak_bin when sub-octave fold hit
        pub noise_threshold: f32,
    }

    pub trait Tap<const N: usize, const HALF_N: usize> {
        fn on_hop(&mut self, report: &HopReport<'_, N, HALF_N>);
    }
    // Blanket impl for closures: FnMut(&HopReport<N, HALF_N>)
}

#[cfg(feature = "tap")]
impl<const N: usize> VocalProcessor<N> {
    /// Identical processing to `process_frame`, plus one `on_hop` callback.
    pub fn process_frame_tapped(
        &mut self,
        input: &[f32; N],
        carrier: Option<&[f32; N]>,
        settings: &MusicalSettings,
        output: &mut [f32; N],
        tap: &mut impl tap::Tap<N, HALF_N>,
    );
}

#[cfg(feature = "tap")]
impl<const N: usize, const BUF: usize> StreamingProcessor<N, BUF> {
    /// Streaming variants — the device-twin GUI shape: feed the same blocks the
    /// Daisy would see, get audio out AND a report per completed hop.
    pub fn process_block_tapped(
        &mut self, input: &[f32], output: &mut [f32],
        settings: &MusicalSettings, tap: &mut impl tap::Tap<N, HALF_N>,
    );
}
```

`HopReport` is `#[non_exhaustive]` **by design**: "show even more of what's going on"
is an explicitly anticipated request, and adding a field to the report is a
non-breaking change. The promotion path for new introspection: add it to `HopReport`
first; it only becomes a knob in `ProcessorConfig` if someone needs to *change* it,
not just see it.

### What the GUI plots from one report

| Panel | Source |
|---|---|
| Input/output waveforms | `input`, `output` |
| Live spectrum (pre/post) | `analysis.magnitudes`, `synthesis.magnitudes` |
| Formant envelope overlay | `envelope` over `analysis.magnitudes` |
| Pitch track + correction pull | `detected_pitch_hz`, `target_pitch_hz`, `pitch_shift_ratio` over `hop_index` |
| YIN CMNDF curve + threshold + chosen lag | `yin` |
| HPS spectrum + sub-octave decision | `hps` |
| Octave-snap events | `raw_pitch_hz` vs `detected_pitch_hz` disagreement |

Ship `examples/scope.rs` (feature `tap` + `std`): CPAL mic in → `process_block_tapped`
→ terminal or egui plots. It doubles as the reference consumer that keeps the report
honest.

## 8. SPSC ring buffer (`spsc`)

Compile-time enforced single-producer/single-consumer; audio-loss semantics kept but
made explicit.

```rust
pub struct RingBuffer<const N: usize> { /* N power of two, compile-time asserted */ }

impl<const N: usize> RingBuffer<N> {
    pub const fn new() -> Self;
    /// Split into halves. Producer/Consumer are Send (not Sync, not Clone);
    /// misuse across more than one context per half no longer compiles.
    pub fn split(&mut self) -> (Producer<'_, N>, Consumer<'_, N>);
}

impl<const N: usize> Producer<'_, N> {
    /// Overwrites oldest unread data when full (audio semantics: glitch, don't block).
    pub fn push(&mut self, v: f32);
    pub fn push_slice(&mut self, v: &[f32]);
    pub fn write_index(&self) -> u32;
}

impl<const N: usize> Consumer<'_, N> {
    /// Returns 0.0 when empty (silence, don't block).
    pub fn pop(&mut self) -> f32;
    pub fn pop_slice(&mut self, out: &mut [f32]);
    pub fn available(&self) -> u32;
    /// STFT helpers, unchanged in spirit from today:
    pub fn block_ending_at<const LEN: usize>(&self, write_idx: u32, dst: &mut [f32; LEN]);
    pub fn add_overlapped<const LEN: usize>(&mut self, frame: &[f32; LEN]);
}
```

All `cfg(feature = "std")` / `cfg(feature = "cortex-m")` synchronization ladders are
removed: under enforced SPSC, relaxed-load/release-store index handoff is sufficient on
every supported target. Result: **identical behavior on all platforms** and the
`cortex-m` and `critical-section` dependencies are dropped entirely.

## 9. Utilities

### `pitch`

```rust
pub struct PitchDetectorConfig { /* HPS noise floor, downsample factors, YIN threshold */ }

/// YIN autocorrelation (time-domain, decimated). Stateless.
pub fn detect_yin(samples: &[f32], sample_rate: f32, cfg: &PitchDetectorConfig) -> Option<f32>;

/// Harmonic product spectrum (frequency-domain).
pub fn detect_hps<const HALF_N: usize>(
    magnitudes: &[f32; HALF_N], bin_width: f32, cfg: &PitchDetectorConfig,
) -> Option<f32>;

/// Quantize a frequency to the nearest note of a key (Key::Chromatic = any semitone).
pub fn quantize_to_key(freq_hz: f32, key: Key) -> f32;
```

`Option<f32>` replaces today's `0.0`-means-unvoiced sentinel.

### `music`

```rust
pub fn note_frequency(note: Note, octave: i8) -> f32;      // A4 = 440.0
pub fn midi_note_frequency(midi_note: u8) -> f32;
pub fn scale_frequencies(key: Key, octave: i8) -> [f32; 8]; // one octave + root
impl Key { pub fn name(&self) -> &'static str; }            // "C Major", …
```

(Today's 24 exported `*_SCALE` consts and the `KEYS` table become an implementation
detail behind these.)

### `fx` — stateless/small-state per-sample effects

```rust
pub fn bitcrush(sample: f32, bits: u8) -> f32;              // u8, not i8

pub struct SampleRateReducer { /* hold counter + held value — today caller-threaded */ }
impl SampleRateReducer {
    pub const fn new() -> Self;
    pub fn process(&mut self, sample: f32, factor: u32) -> f32;
}

pub fn normalize(sample: f32, target_peak: f32) -> f32;     // one copy, not two
```

### `osc`

```rust
pub enum Waveform { Sine, Saw, Square, Triangle }

pub struct Oscillator { ... }
impl Oscillator {
    pub const fn new(sample_rate: f32) -> Self;
    pub fn set_frequency(&mut self, hz: f32);
    pub fn set_waveform(&mut self, w: Waveform);
    pub fn next(&mut self) -> f32;    // sine via internal fast-math facade, not libm
    pub fn fill(&mut self, out: &mut [f32]);
}
```

### `raw`

Today's `process_*_generic` free functions, `#[doc(hidden)]`, explicitly semver-exempt.
Escape hatch for callers who need to own state placement at a finer grain than
`VocalProcessor` allows.

## 10. Feature flags

| Feature | Default | Adds |
|---|---|---|
| *(none)* | — | Full `no_std` core: processor, streaming, blocks, spsc, pitch, music, fx, osc |
| `std` | off | `std::error::Error` impl, `Display` niceties |
| `log` | off | `log` crate tracing at hop boundaries (was `debug-logging`, unwired) |
| `serde` | off | `Serialize`/`Deserialize` on `ProcessorConfig`, `MusicalSettings`, `Key`, etc. — preset save/load in desktop hosts |
| `defmt` | off | `defmt::Format` on public types — idiomatic embedded logging (RTT) without `core::fmt` bloat |
| `tap` | off | Diagnostics tap (§7): per-hop `HopReport` with every pipeline stage borrowed out for metering/plotting GUIs. Compiled out entirely when off — the Daisy build pays zero |

Removed: `embedded` (empty default), `cortex-m` (no longer needed, §8),
`cepstral-smoothing` / `formant-shifting` (unwired today; cepstral envelope extraction
becomes always-on internals — it's required for correct formant handling, not optional).

## 11. Canonical examples (ship in `examples/`, compile in CI)

**Desktop, streaming** (`examples/wav_autotune.rs`):

```rust
let config = ProcessorConfig::new(48_000.0);
let mut sp = StreamingProcessor::<1024, 4096>::new(config);
let settings = MusicalSettings {
    mode: ProcessingMode::PitchCorrect,
    key: Key::Major(Note::C),
    ..Default::default()
};
for block in wav_samples.chunks(256) {
    sp.write(block, &settings);
    sp.read(&mut out_block[..block.len()]);
    writer.extend(&out_block[..block.len()]);
}
```

**Embedded, split-context** (`examples/rtic_skeleton.rs`, `--target thumbv7em-none-eabihf` build-only):

```rust
// ISR (audio DMA):   producer.push(sample);  consumer_out.pop() → DAC
// FFT task (lower priority), every HOP samples:
let mut frame = [0.0f32; 1024];
in_consumer.block_ending_at(in_producer_idx, &mut frame);
processor.process_frame(&frame, None, &settings, &mut synth);
out_producer.add_overlapped(&synth);
```

## 12. Migration map (0.1.x → 0.2.0)

| 0.1.x | 0.2.0 |
|---|---|
| `process_vocal_effects_1024(9 args)` | `VocalProcessor::<1024>::process_frame(4 args)` |
| `VocalEffectsConfig { pub fields }` | `ProcessorConfig::new(sr).hop_ratio(..)?` |
| `MusicalSettings { key: 3, formant: 1, .. }` | `{ key: Key::Major(Note::A), formant: FormantMode::Shift(0.5) }` |
| `settings.octave = 4` (keypad-row encoding) | `settings.pitch_shift_semitones = 12.0` |
| `ProcessingMode::{PitchControl, Dry}` | `ProcessingMode::{PitchCorrect, PitchShift}` |
| `RingBuffer` shared by `&self` | `RingBuffer::split() → (Producer, Consumer)` |
| `dsp::{bitcrush, sample_rate_reduce, normalize_sample}` | `fx::{bitcrush, SampleRateReducer, normalize}` |
| `find_pitch_yin(..) -> f32` (0.0 = unvoiced) | `pitch::detect_yin(..) -> Option<f32>` |
| features `embedded`, `cortex-m`, `debug-logging` | *(removed)*, *(removed)*, `log` |

## 13. Cross-cutting API contracts

Guarantees that aren't visible in any single signature but that integrators depend on.
Each is a documented, CI-enforced promise from 0.2.0 — these hold even while the API
shape is still allowed to move.

### Real-time safety
- **No panics in processing paths.** `process_frame`, `write`/`read`/`process_block`,
  and every `blocks::*` function are panic-free for all input values (including NaN,
  ±inf, denormals — garbage in, silence/garbage out, never abort). Enforced in CI by
  fuzzing the process paths with adversarial input plus a link-time check that the
  release rlib's process symbols pull in no panic machinery.
- **No allocation, no locks, no syscalls** anywhere in processing paths (constructors
  may do what they like). This is what "real-time safe" means on both an ISR and a
  CoreAudio thread.
- **Failure = silence, not errors.** Underruns, unvoiced input, and not-yet-primed
  pipelines produce zeros. Real-time code has no one to report errors to.

### Threading
- `VocalProcessor`, `StreamingProcessor`, `StftAnalyzer/Synthesizer`, `Oscillator`:
  `Send` (move them to the audio thread) but **not** `Sync` — one instance, one
  context; no hidden interior mutability.
- `spsc::Producer`/`Consumer`: each `Send`, the pair usable from two contexts — that's
  the point.
- Settings/config types: `Send + Sync + Copy` plain data; build them anywhere, hand
  them to the audio thread by value.

### Channels
- **The library is mono by design.** Vocal pitch processing is inherently monophonic;
  stereo/multichannel is N independent processors (document the two-instance stereo
  pattern in an example). No channel-count parameter will be added — this is a
  contract, not a gap.

### Numerics
- f32 throughout; f64 has no place on Cortex-M7 (single-precision FPU) and no audible
  benefit here.
- **Denormal policy:** feedback state (phase accumulators, envelope caches, held
  samples) is flushed to zero below 1e-15 internally. On x86 hosts denormals are a
  100× per-op penalty; on ARM the FPU flushes by default. The library guarantees its
  *state* never denormalizes; input denormals are the host's business.
- All public plain-data types derive `Debug, Clone, Copy, PartialEq` (Rust API
  guidelines C-COMMON-TRAITS); processors derive `Debug` (redacted large arrays) and
  `Clone` (cheap way to preallocate a reset template).

## 14. Use-case coverage matrix

The spec is checked against these personas; every row must have a complete API path.
When a future change forces a trade-off between rows, **row 1 wins** (design
principle 6).

| # | Persona | API path | Knobs they touch |
|---|---|---|---|
| 1 | **Synthphone (Daisy Seed)** — the reason this crate exists | `spsc` + `VocalProcessor` split-context (§5/§11); `Smoothed` envelope where the budget demands | Modes, key, formant, targets from MIDI; `envelope_method`, `harmony_envelope_refresh_hops` to fit the 5.3 ms hop budget |
| 2 | **Voice changer** (desktop/RPi toy, cosplay, streaming) | `StreamingProcessor::process_block[_in_place]`, mode `PitchShift` | `pitch_shift_semitones` (continuous ± cents), `FormantMode::Shift` independent of pitch, `mix`; robot/whisper via `blocks` (§6.4) |
| 3 | **Autotune / music maker** (plugin, DAW tool) | `StreamingProcessor` in a plugin `process()`; `serde` presets | `correction_strength` (hard-tune ↔ subtle), `transition_speed`, `Key::Custom(ScaleMask)` for pentatonic/modal scales, `forced_note` for melody control, `mix` |
| 4 | **Harmonizer / vocoder instrument** | `write_with_carrier` with own synth as carrier; custom voicing via `blocks::add_shifted` per-voice gains | `target_frequencies` (any voice count), `envelope_method`, per-voice gain in `blocks` |
| 5 | **Tuner / analyzer / visualizer** | `analyze_frame` (no synthesis, ~half cost) + `pitch::*`, `SpectralFrame` bins for display | `PitchDetectorConfig` (range, thresholds), `reference_pitch_hz` (A≠440 tunings) |
| 6 | **Sound designer / circuit-bender** | Any of the above + `blocks`; extreme values are legal by design principle 7 | *Everything*: `magnitude_threshold` as a spectral gate, `limiter_threshold` > 1.0, 8× formant warps, `hop_ratio` extremes, YIN thresholds on non-vocal material |
| 7 | **Device-twin GUI / metrics dashboard** — a desktop app that behaves like the Daisy but visualizes the internals (waveforms, spectra, formant envelope, pitch tracks, YIN/HPS traces) | `StreamingProcessor::process_block_tapped` with feature `tap` (§7); same blocks the Daisy sees, plus one `HopReport` per hop | Read-only access to every stage; `HopReport` is `#[non_exhaustive]` so "show more" is always a non-breaking addition |

Gaps this matrix closed (all now in §1/§2): continuous `pitch_shift_semitones` (row 2 —
octave-only transpose was Synthphone keypad leakage), `mix` (rows 2–4 — wet/dry was
hardcoded per mode), `Key::Custom(ScaleMask)` (row 3 — 24 preset keys aren't a music
tool), `EnvelopeMethod` (rows 1 vs 4 — the Daisy/desktop CPU trade-off was previously
an internal hardcode), full `PitchDetectorConfig` exposure (rows 5–6), and the
reject-crashes-not-weird-sounds validation policy (row 6).

## 15. Stability policy

- 0.2.0 is a **pre-1.0 release by intent**: standard 0.x semver applies (breaking
  changes bump the minor version, 0.2.x patches never break). We aim to hold the shape
  in this spec through 0.2.x, but reserve the right to learn from the firmware
  migration and desktop dogfooding.
- 1.0 is cut when the Synthphone has shipped on this API and it has stopped moving —
  a milestone, not a date.
- `raw`: exempt at every version, `#[doc(hidden)]`.
- All public structs/enums that may grow: `#[non_exhaustive]` from 0.2.0.
- CI runs `cargo semver-checks` from 0.2.0 (it understands 0.x rules); the §13
  real-time contracts are enforced from 0.2.0 regardless.

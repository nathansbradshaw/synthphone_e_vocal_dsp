#![cfg_attr(not(feature = "std"), no_std)]
#![cfg_attr(docsrs, feature(doc_cfg))]

// Core modules
pub mod config;
pub mod error;
pub mod state;

// Audio processing modules
pub mod audio;
pub mod vocal_effects;

// Buffer management
pub mod ring_buffer;

// Utility modules
pub mod math;

pub mod dsp;
pub mod effects;

// Re-export main API
pub use config::VocalEffectsConfig;
pub use error::VocalEffectsError;
pub use state::{MusicalSettings, ProcessingMode};

// Re-export commonly used functions
pub use vocal_effects::{
    VocalStreamState, process_vocal_effects_1024_stream,
    process_vocal_effects_512, process_vocal_effects_1024, process_vocal_effects_1024_with_state,
    process_vocal_effects_2048, process_vocal_effects_4096,
};

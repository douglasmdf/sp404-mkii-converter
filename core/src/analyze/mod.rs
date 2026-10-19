//! Audio analysis for the key/BPM prefix: note, chord, key and tempo of a sample.
//! Pure Rust, deterministic (no randomness, no dependence on thread count).

use serde::Serialize;

pub mod features;
pub mod pitch;
pub mod signal;
#[cfg(test)]
pub(crate) mod synth;

/// Every file is analysed at this rate, whatever its source rate.
pub const ANALYSIS_RATE: u32 = 22_050;
/// Only the start of a file is decoded and analysed.
pub const MAX_ANALYSIS_SECONDS: f64 = 180.0;
/// Bumped whenever thresholds or profiles change, so logs show which analyzer named a file.
pub const ANALYZER_VERSION: u32 = 1;

/// A detected value and whether the analyzer trusts it enough to name the file with it.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Estimate<T> {
    pub value: T,
    pub confident: bool,
}

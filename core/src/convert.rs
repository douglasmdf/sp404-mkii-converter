//! One file in, one SP-404 WAV out.

use std::path::Path;

use serde::Serialize;

use crate::config::Settings;
use crate::error::Result;
use crate::process::TARGET_RATE;
use crate::{decode, encode, process};

/// The SP-404 MKII's limit for a single sample.
pub const MAX_SAMPLE_SECONDS: f64 = 16.0 * 60.0;

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Warning {
    Clipped { samples: u64 },
    TooLong { seconds: f64 },
    Downmixed { channels: usize },
}

pub fn collect_warnings(frames: usize, clipped_samples: u64, downmixed_from: Option<usize>) -> Vec<Warning> {
    let mut warnings = Vec::new();
    if let Some(channels) = downmixed_from {
        warnings.push(Warning::Downmixed { channels });
    }
    if clipped_samples > 0 {
        warnings.push(Warning::Clipped { samples: clipped_samples });
    }
    let seconds = frames as f64 / TARGET_RATE as f64;
    if seconds > MAX_SAMPLE_SECONDS {
        warnings.push(Warning::TooLong { seconds });
    }
    warnings
}

pub fn convert_file(source: &Path, dest: &Path, settings: &Settings) -> Result<Vec<Warning>> {
    let decoded = decode::decode_file(source)?;
    let processed = process::process(decoded, settings.downmix_multichannel)?;
    let frames = processed.interleaved.len() / usize::from(processed.channels.max(1));
    let warnings = collect_warnings(frames, processed.clipped_samples, processed.downmixed_from);
    encode::write_wav(dest, processed.channels, &processed.interleaved)?;
    Ok(warnings)
}

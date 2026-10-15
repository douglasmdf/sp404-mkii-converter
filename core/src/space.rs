//! Rough free-space check before converting. Conservative on purpose.

use std::fs;
use std::path::{Path, PathBuf};

use crate::decode::{probe_info, AudioInfo};
use crate::error::{io_err, CoreError, Result};
use crate::process::TARGET_RATE;
use crate::scan::{JobKind, Plan};

const WAV_HEADER_BYTES: u64 = 44;
/// Used when the header does not tell the length (e.g. some MP3s): a 128 kbps MP3
/// grows about 12× as 16-bit / 48 kHz stereo WAV.
const UNKNOWN_LENGTH_GROWTH: u64 = 12;

pub fn estimate_output_bytes(plan: &Plan) -> u64 {
    plan.jobs()
        .filter(|job| !job.already_exists)
        .map(|job| match job.kind {
            JobKind::Convert => estimate_wav_bytes(&job.source),
            JobKind::Copy => fs::metadata(&job.source).map(|m| m.len()).unwrap_or(0),
            JobKind::Unsupported => 0,
        })
        .sum()
}

fn estimate_wav_bytes(source: &Path) -> u64 {
    match probe_info(source) {
        Ok(AudioInfo { frames: Some(frames), sample_rate: Some(rate), channels: Some(channels) }) if rate > 0 => {
            let out_frames = (frames as f64 * TARGET_RATE as f64 / rate as f64).ceil() as u64;
            WAV_HEADER_BYTES + out_frames * channels.min(2) as u64 * 2
        }
        _ => fs::metadata(source)
            .map(|m| m.len().saturating_mul(UNKNOWN_LENGTH_GROWTH))
            .unwrap_or(0),
    }
}

pub fn ensure_space(needed: u64, available: u64) -> Result<()> {
    if needed > available {
        return Err(CoreError::InsufficientSpace { needed, available });
    }
    Ok(())
}

/// Every pack's destination volume must fit the whole batch (simple and conservative).
pub fn check_free_space(plan: &Plan) -> Result<()> {
    let needed = estimate_output_bytes(plan);
    for pack in &plan.packs {
        let probe_dir = existing_ancestor(&pack.dest_root);
        let available = fs4::available_space(&probe_dir).map_err(io_err(&probe_dir))?;
        ensure_space(needed, available)?;
    }
    Ok(())
}

fn existing_ancestor(path: &Path) -> PathBuf {
    let mut current = path;
    while !current.exists() {
        match current.parent() {
            Some(parent) => current = parent,
            None => break,
        }
    }
    current.to_path_buf()
}

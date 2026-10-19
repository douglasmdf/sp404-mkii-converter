//! The mono, fixed-rate, silence-trimmed signal every analysis step reads.

use std::path::Path;

use super::{ANALYSIS_RATE, MAX_ANALYSIS_SECONDS};
use crate::decode::decode_file_limited;
use crate::error::Result;
use crate::process::resample;

/// Leading/trailing samples quieter than this (relative to the peak) are trimmed.
const TRIM_DB: f32 = -60.0;

#[derive(Debug, Clone, PartialEq)]
pub struct Signal {
    pub samples: Vec<f32>,
    pub rate: u32,
    /// Duration after trimming leading/trailing silence.
    pub source_seconds: f64,
    /// Duration of the decoded audio before trimming (at most `MAX_ANALYSIS_SECONDS`). A loop
    /// is cut to a whole number of beats including any silence at its end, so the tempo
    /// candidates come from this length.
    pub file_seconds: f64,
}

/// Decodes at most `MAX_ANALYSIS_SECONDS`, averages the channels to mono, resamples to
/// `ANALYSIS_RATE` and trims silence at both ends. A silent or empty file gives an empty signal.
pub fn load(path: &Path) -> Result<Signal> {
    let decoded = decode_file_limited(path, Some(MAX_ANALYSIS_SECONDS))?;
    let buffer = decoded.buffer;
    let max_frames = (MAX_ANALYSIS_SECONDS * buffer.sample_rate as f64) as usize;
    let frames = buffer.frames().min(max_frames);
    let n_ch = buffer.channels.len() as f32;
    let mono: Vec<f32> = (0..frames).map(|i| buffer.channels.iter().map(|c| c[i]).sum::<f32>() / n_ch).collect();
    let file_seconds = frames as f64 / buffer.sample_rate as f64;
    let mut samples = resample(&[mono], buffer.sample_rate, ANALYSIS_RATE)?.swap_remove(0);
    trim_silence(&mut samples);
    let source_seconds = samples.len() as f64 / ANALYSIS_RATE as f64;
    Ok(Signal { samples, rate: ANALYSIS_RATE, source_seconds, file_seconds })
}

fn trim_silence(samples: &mut Vec<f32>) {
    let peak = samples.iter().fold(0f32, |m, s| m.max(s.abs()));
    let threshold = peak * 10f32.powf(TRIM_DB / 20.0);
    let loud = |s: &f32| peak > 0.0 && s.abs() > threshold;
    match (samples.iter().position(loud), samples.iter().rposition(loud)) {
        (Some(first), Some(last)) => {
            samples.truncate(last + 1);
            samples.drain(..first);
        }
        _ => samples.clear(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analyze::ANALYSIS_RATE;

    #[test]
    fn load_resamples_and_trims() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.wav");
        let spec = hound::WavSpec { channels: 2, sample_rate: 48_000, bits_per_sample: 16, sample_format: hound::SampleFormat::Int };
        let mut writer = hound::WavWriter::create(&path, spec).unwrap();
        for i in 0..72_000 {
            let v = if i < 24_000 { 0.0 } else { 0.5 * (std::f32::consts::TAU * 440.0 * i as f32 / 48_000.0).sin() };
            let s = (v * 32767.0).round() as i16;
            writer.write_sample(s).unwrap();
            writer.write_sample(s).unwrap();
        }
        writer.finalize().unwrap();
        let signal = load(&path).unwrap();
        assert_eq!(signal.rate, ANALYSIS_RATE);
        assert!((0.95..=1.05).contains(&signal.source_seconds), "{}", signal.source_seconds);
        assert!((signal.file_seconds - 1.5).abs() < 1e-6, "{}", signal.file_seconds);
        assert!((signal.samples.len() as f64 / ANALYSIS_RATE as f64 - signal.source_seconds).abs() < 1e-6);
    }
}

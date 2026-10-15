#![allow(dead_code)]

use std::path::{Path, PathBuf};

pub struct WavFormat {
    pub sample_rate: u32,
    pub channels: u16,
    pub bits: u16,
    pub float: bool,
}

/// Writes the same sine wave on every channel.
pub fn write_sine_wav(path: &Path, fmt: &WavFormat, freq: f32, amplitude: f32, frames: usize) {
    let spec = hound::WavSpec {
        channels: fmt.channels,
        sample_rate: fmt.sample_rate,
        bits_per_sample: fmt.bits,
        sample_format: if fmt.float { hound::SampleFormat::Float } else { hound::SampleFormat::Int },
    };
    let mut writer = hound::WavWriter::create(path, spec).unwrap();
    let max = ((1i64 << (fmt.bits - 1)) - 1) as f32;
    for i in 0..frames {
        let v = amplitude * (2.0 * std::f32::consts::PI * freq * i as f32 / fmt.sample_rate as f32).sin();
        for _ in 0..fmt.channels {
            if fmt.float {
                writer.write_sample(v).unwrap();
            } else {
                writer.write_sample((v * max).round() as i32).unwrap();
            }
        }
    }
    writer.finalize().unwrap();
}

/// Writes interleaved 16-bit samples as they are.
pub fn write_i16_wav(path: &Path, sample_rate: u32, channels: u16, samples: &[i16]) {
    let spec = hound::WavSpec { channels, sample_rate, bits_per_sample: 16, sample_format: hound::SampleFormat::Int };
    let mut writer = hound::WavWriter::create(path, spec).unwrap();
    for &s in samples {
        writer.write_sample(s).unwrap();
    }
    writer.finalize().unwrap();
}

pub fn read_wav(path: &Path) -> (hound::WavSpec, Vec<i16>) {
    let mut reader = hound::WavReader::open(path).unwrap();
    let spec = reader.spec();
    let samples = reader.samples::<i16>().map(|s| s.unwrap()).collect();
    (spec, samples)
}

/// `(chunk id, chunk size)` for every top-level chunk of a RIFF/WAVE file.
pub fn riff_chunks(path: &Path) -> Vec<(String, u32)> {
    let bytes = std::fs::read(path).unwrap();
    assert_eq!(&bytes[0..4], b"RIFF");
    assert_eq!(&bytes[8..12], b"WAVE");
    let mut chunks = Vec::new();
    let mut pos = 12;
    while pos + 8 <= bytes.len() {
        let id = String::from_utf8_lossy(&bytes[pos..pos + 4]).into_owned();
        let size = u32::from_le_bytes(bytes[pos + 4..pos + 8].try_into().unwrap());
        chunks.push((id, size));
        pos += 8 + size as usize + (size as usize & 1);
    }
    chunks
}

pub fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests").join("fixtures").join(name)
}

//! Writes output files atomically: data goes to `<name>.part`, then is renamed, so an
//! interrupted run never leaves a half-written file under the final name.

use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};

use hound::{SampleFormat, WavSpec, WavWriter};

use crate::error::{io_err, CoreError, Result};
use crate::process::TARGET_RATE;

pub fn part_path(dest: &Path) -> PathBuf {
    let mut name: OsString = dest.as_os_str().to_owned();
    name.push(".part");
    PathBuf::from(name)
}

/// 16-bit / 48 kHz PCM. hound writes a plain 16-byte `fmt ` chunk for ≤2 channels at
/// 16 bits, and no metadata chunks.
pub fn write_wav(dest: &Path, channels: u16, interleaved: &[i16]) -> Result<()> {
    let part = part_path(dest);
    let spec = WavSpec { channels, sample_rate: TARGET_RATE, bits_per_sample: 16, sample_format: SampleFormat::Int };
    let written = (|| -> std::result::Result<(), hound::Error> {
        let mut writer = WavWriter::create(&part, spec)?;
        for &sample in interleaved {
            writer.write_sample(sample)?;
        }
        writer.finalize()
    })();
    if let Err(e) = written {
        let _ = fs::remove_file(&part);
        return Err(CoreError::Encode(e.to_string()));
    }
    fs::rename(&part, dest).map_err(io_err(dest))
}

pub fn copy_file(source: &Path, dest: &Path) -> Result<()> {
    let part = part_path(dest);
    if let Err(source_err) = fs::copy(source, &part) {
        let _ = fs::remove_file(&part);
        return Err(CoreError::Io { path: dest.to_path_buf(), source: source_err });
    }
    fs::rename(&part, dest).map_err(io_err(dest))
}

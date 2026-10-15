mod common;

use std::fs;

use common::*;
use sp404_core::config::Settings;
use sp404_core::convert::{collect_warnings, convert_file, Warning, MAX_SAMPLE_SECONDS};
use sp404_core::encode::{copy_file, part_path, write_wav};

fn assert_sp404_wav(path: &std::path::Path, channels: u16) -> Vec<i16> {
    let chunks = riff_chunks(path);
    let ids: Vec<&str> = chunks.iter().map(|(id, _)| id.as_str()).collect();
    assert_eq!(ids, ["fmt ", "data"], "{}", path.display());
    assert_eq!(chunks[0].1, 16, "fmt chunk must be plain PCM");
    let bytes = fs::read(path).unwrap();
    assert_eq!(u16::from_le_bytes([bytes[20], bytes[21]]), 1, "format tag must be PCM");
    let (spec, samples) = read_wav(path);
    assert_eq!(spec.sample_rate, 48_000);
    assert_eq!(spec.bits_per_sample, 16);
    assert_eq!(spec.sample_format, hound::SampleFormat::Int);
    assert_eq!(spec.channels, channels);
    assert!(!part_path(path).exists());
    samples
}

#[test]
fn write_wav_produces_clean_pcm_and_no_part_file() {
    let dir = tempfile::tempdir().unwrap();
    let dest = dir.path().join("out.wav");
    write_wav(&dest, 2, &[1, 2, 3, 4]).unwrap();
    assert_eq!(assert_sp404_wav(&dest, 2), vec![1, 2, 3, 4]);
}

#[test]
fn write_wav_into_missing_folder_fails_cleanly() {
    let dir = tempfile::tempdir().unwrap();
    let dest = dir.path().join("missing").join("out.wav");
    assert!(write_wav(&dest, 1, &[0]).is_err());
    assert!(!dest.exists());
    assert!(!part_path(&dest).exists());
}

#[test]
fn copy_file_copies_bytes() {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("art.jpg");
    let dest = dir.path().join("copy.jpg");
    fs::write(&src, b"jpeg bytes").unwrap();
    copy_file(&src, &dest).unwrap();
    assert_eq!(fs::read(&dest).unwrap(), b"jpeg bytes");
    assert!(!part_path(&dest).exists());
}

#[test]
fn generated_wavs_convert_to_sp404_format() {
    let dir = tempfile::tempdir().unwrap();
    let cases = [
        ("24bit_44k_stereo.wav", WavFormat { sample_rate: 44_100, channels: 2, bits: 24, float: false }, 4_410, 2u16),
        ("float_96k_mono.wav", WavFormat { sample_rate: 96_000, channels: 1, bits: 32, float: true }, 9_600, 1),
        ("8bit_22k_mono.wav", WavFormat { sample_rate: 22_050, channels: 1, bits: 8, float: false }, 2_205, 1),
        ("16bit_48k_6ch.wav", WavFormat { sample_rate: 48_000, channels: 6, bits: 16, float: false }, 4_800, 2),
    ];
    for (name, fmt, frames, out_channels) in cases {
        let src = dir.path().join(name);
        let dest = dir.path().join(format!("out_{name}"));
        write_sine_wav(&src, &fmt, 440.0, 0.5, frames);
        convert_file(&src, &dest, &Settings::default()).unwrap_or_else(|e| panic!("{name}: {e}"));
        let samples = assert_sp404_wav(&dest, out_channels);
        assert_eq!(samples.len(), 4_800 * out_channels as usize, "{name}");
    }
}

#[test]
fn fixtures_convert_to_sp404_format() {
    let dir = tempfile::tempdir().unwrap();
    for name in ["sine.flac", "sine.mp3", "sine.m4a", "sine.ogg", "sine.aiff", "sine.caf"] {
        let dest = dir.path().join(format!("{name}.wav"));
        convert_file(&fixture(name), &dest, &Settings::default()).unwrap_or_else(|e| panic!("{name}: {e}"));
        let frames = assert_sp404_wav(&dest, 2).len() / 2;
        assert!((21_000..=29_000).contains(&frames), "{name}: {frames} frames");
    }
}

#[test]
fn already_compliant_file_is_bit_identical() {
    // Review Focus 5
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("ok.wav");
    let dest = dir.path().join("out.wav");
    let samples: Vec<i16> = (0..4000i32).map(|i| ((i * 7919) % 65536 - 32768) as i16).collect();
    write_i16_wav(&src, 48_000, 2, &samples);
    let warnings = convert_file(&src, &dest, &Settings::default()).unwrap();
    assert!(warnings.is_empty());
    assert_eq!(assert_sp404_wav(&dest, 2), samples);
}

#[test]
fn zero_length_file_converts_to_empty_wav() {
    // Review Focus 4
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("empty.wav");
    let dest = dir.path().join("out.wav");
    write_i16_wav(&src, 44_100, 2, &[]);
    convert_file(&src, &dest, &Settings::default()).unwrap();
    assert!(assert_sp404_wav(&dest, 2).is_empty());
}

#[test]
fn leftover_part_file_is_replaced() {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("a.wav");
    let dest = dir.path().join("out.wav");
    write_i16_wav(&src, 48_000, 1, &[1, 2, 3]);
    fs::write(part_path(&dest), b"garbage from a crash").unwrap();
    convert_file(&src, &dest, &Settings::default()).unwrap();
    assert_eq!(assert_sp404_wav(&dest, 1), vec![1, 2, 3]);
}

#[test]
fn multichannel_and_clipping_produce_warnings() {
    let dir = tempfile::tempdir().unwrap();
    let six = dir.path().join("six.wav");
    write_sine_wav(&six, &WavFormat { sample_rate: 48_000, channels: 6, bits: 16, float: false }, 440.0, 0.5, 480);
    let w = convert_file(&six, &dir.path().join("six_out.wav"), &Settings::default()).unwrap();
    assert_eq!(w, vec![Warning::Downmixed { channels: 6 }]);

    let hot = dir.path().join("hot.wav");
    write_sine_wav(&hot, &WavFormat { sample_rate: 48_000, channels: 1, bits: 32, float: true }, 440.0, 1.5, 480);
    let w = convert_file(&hot, &dir.path().join("hot_out.wav"), &Settings::default()).unwrap();
    assert!(matches!(w.as_slice(), [Warning::Clipped { samples }] if *samples > 0));
}

#[test]
fn downmix_off_fails_multichannel() {
    let dir = tempfile::tempdir().unwrap();
    let six = dir.path().join("six.wav");
    write_sine_wav(&six, &WavFormat { sample_rate: 48_000, channels: 6, bits: 16, float: false }, 440.0, 0.5, 48);
    let mut s = Settings::default();
    s.downmix_multichannel = false;
    let dest = dir.path().join("out.wav");
    assert!(convert_file(&six, &dest, &s).is_err());
    assert!(!dest.exists());
}

#[test]
fn long_files_get_too_long_warning() {
    let frames = (MAX_SAMPLE_SECONDS as usize + 1) * 48_000;
    assert!(matches!(collect_warnings(frames, 0, None).as_slice(), [Warning::TooLong { .. }]));
    assert!(collect_warnings(48_000, 0, None).is_empty());
}

#[test]
fn warnings_serialize_with_kind_tag() {
    let json = serde_json::to_string(&Warning::Clipped { samples: 3 }).unwrap();
    assert_eq!(json, r#"{"kind":"clipped","samples":3}"#);
    let json = serde_json::to_string(&Warning::TooLong { seconds: 1.5 }).unwrap();
    assert_eq!(json, r#"{"kind":"tooLong","seconds":1.5}"#);
}

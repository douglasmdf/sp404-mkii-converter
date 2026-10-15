mod common;

use common::*;
use sp404_core::decode::{decode_file, probe_info};

#[test]
fn wav_16bit_is_exact_and_lossless() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("a.wav");
    let samples: Vec<i16> = vec![0, 1000, -1000, 32767, -32768, 5];
    write_i16_wav(&path, 44_100, 2, &samples);
    let d = decode_file(&path).unwrap();
    assert!(d.exact_16bit);
    assert_eq!(d.buffer.sample_rate, 44_100);
    assert_eq!(d.buffer.channels.len(), 2);
    assert_eq!(d.buffer.frames(), 3);
    let back: Vec<i16> = (0..3)
        .flat_map(|i| [d.buffer.channels[0][i], d.buffer.channels[1][i]])
        .map(|x| (x * 32768.0) as i16)
        .collect();
    assert_eq!(back, samples);
}

#[test]
fn high_resolution_wavs_are_not_exact() {
    let dir = tempfile::tempdir().unwrap();
    let p24 = dir.path().join("24.wav");
    let pf = dir.path().join("f.wav");
    write_sine_wav(&p24, &WavFormat { sample_rate: 96_000, channels: 2, bits: 24, float: false }, 440.0, 0.5, 960);
    write_sine_wav(&pf, &WavFormat { sample_rate: 48_000, channels: 1, bits: 32, float: true }, 440.0, 0.5, 480);
    let d24 = decode_file(&p24).unwrap();
    assert!(!d24.exact_16bit);
    assert_eq!((d24.buffer.sample_rate, d24.buffer.channels.len(), d24.buffer.frames()), (96_000, 2, 960));
    let df = decode_file(&pf).unwrap();
    assert!(!df.exact_16bit);
    assert_eq!((df.buffer.channels.len(), df.buffer.frames()), (1, 480));
}

#[test]
fn six_channel_wav_keeps_all_channels() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("51.wav");
    write_sine_wav(&path, &WavFormat { sample_rate: 48_000, channels: 6, bits: 16, float: false }, 440.0, 0.5, 100);
    assert_eq!(decode_file(&path).unwrap().buffer.channels.len(), 6);
}

#[test]
fn zero_length_wav_decodes_to_no_frames() {
    // Review Focus 4
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("empty.wav");
    write_i16_wav(&path, 44_100, 2, &[]);
    let d = decode_file(&path).unwrap();
    assert_eq!(d.buffer.channels.len(), 2);
    assert_eq!(d.buffer.frames(), 0);
    assert_eq!(d.buffer.sample_rate, 44_100);
}

#[test]
fn compressed_and_other_container_fixtures_decode() {
    for (name, exact) in [
        ("sine.flac", true),
        ("sine.aiff", true),
        ("sine.caf", true),
        ("sine.mp3", false),
        ("sine.m4a", false),
        ("sine.ogg", false),
    ] {
        let d = decode_file(&fixture(name)).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_eq!(d.buffer.sample_rate, 44_100, "{name}");
        assert_eq!(d.buffer.channels.len(), 2, "{name}");
        let frames = d.buffer.frames();
        assert!((19_000..=27_000).contains(&frames), "{name}: {frames} frames");
        let peak = d.buffer.channels[0].iter().fold(0f32, |m, s| m.max(s.abs()));
        assert!(peak > 0.05, "{name}: peak {peak}");
        assert_eq!(d.exact_16bit, exact, "{name}");
    }
}

#[test]
fn garbage_file_is_an_error() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("broken.wav");
    std::fs::write(&path, b"definitely not audio").unwrap();
    assert!(decode_file(&path).is_err());
}

#[test]
fn probe_reads_header_only() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("a.wav");
    write_sine_wav(&path, &WavFormat { sample_rate: 44_100, channels: 2, bits: 24, float: false }, 440.0, 0.5, 4_410);
    let info = probe_info(&path).unwrap();
    assert_eq!(info.frames, Some(4_410));
    assert_eq!(info.sample_rate, Some(44_100));
    assert_eq!(info.channels, Some(2));
}

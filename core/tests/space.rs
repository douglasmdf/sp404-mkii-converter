mod common;

use std::fs;

use common::*;
use sp404_core::config::Settings;
use sp404_core::scan::build_plan;
use sp404_core::space::{check_free_space, ensure_space, estimate_output_bytes};
use sp404_core::CoreError;

#[test]
fn estimate_counts_converted_audio_and_copies_but_not_existing() {
    let tmp = tempfile::tempdir().unwrap();
    let pack = tmp.path().join("Pack");
    fs::create_dir_all(&pack).unwrap();
    write_sine_wav(&pack.join("a.wav"), &WavFormat { sample_rate: 44_100, channels: 2, bits: 24, float: false }, 440.0, 0.5, 44_100);
    fs::write(pack.join("notes.txt"), b"hello").unwrap();
    let out = tmp.path().join("out");
    let plan = build_plan(&[pack.clone()], Some(&out), &Settings::default()).unwrap();
    assert_eq!(estimate_output_bytes(&plan), 44 + 48_000 * 2 * 2 + 5);

    fs::create_dir_all(out.join("Pack")).unwrap();
    fs::write(out.join("Pack/notes.txt"), b"old").unwrap();
    let plan = build_plan(&[pack], Some(&out), &Settings::default()).unwrap();
    assert_eq!(estimate_output_bytes(&plan), 44 + 48_000 * 2 * 2);
    check_free_space(&plan).unwrap();
}

#[test]
fn ensure_space_rejects_when_short() {
    assert!(ensure_space(10, 10).is_ok());
    assert!(matches!(
        ensure_space(11, 10),
        Err(CoreError::InsufficientSpace { needed: 11, available: 10 })
    ));
}

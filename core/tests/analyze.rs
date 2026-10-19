mod common;

use common::*;
use sp404_core::analyze::{analyze_file, NameHints};
use sp404_core::tags::{PitchClass, SampleKind};

#[test]
fn analyze_file_on_wav() {
    let tmp = tempfile::tempdir().unwrap();
    let fmt = WavFormat { sample_rate: 44_100, channels: 1, bits: 16, float: false };
    let sine = tmp.path().join("sine.wav");
    write_sine_wav(&sine, &fmt, 220.0, 0.5, 44_100);
    let found = analyze_file(&sine, &NameHints::default()).unwrap();
    assert_eq!(found.kind.value, SampleKind::Note);
    assert_eq!(found.note.map(|n| n.value), Some(PitchClass(9)));

    let empty = tmp.path().join("empty.wav");
    write_i16_wav(&empty, 44_100, 1, &[]);
    assert!(analyze_file(&empty, &NameHints::default()).is_err());
}

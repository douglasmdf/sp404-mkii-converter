mod common;

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::Mutex;

use common::*;
use sp404_core::config::Settings;
use sp404_core::runner::{run, Event, FileStatus, LOG_FILE_NAME};
use sp404_core::scan::{build_plan, Plan};
use sp404_core::undo::{undo, Manifest};

fn make_pack(root: &Path) -> PathBuf {
    let pack = root.join("Pack");
    fs::create_dir_all(pack.join("Drums")).unwrap();
    let fmt = WavFormat { sample_rate: 44_100, channels: 2, bits: 24, float: false };
    write_sine_wav(&pack.join("Drums/kick.wav"), &fmt, 60.0, 0.5, 4_410);
    write_sine_wav(&pack.join("Drums/snare.wav"), &fmt, 200.0, 0.5, 4_410);
    fs::write(pack.join("artwork.jpg"), b"jpeg").unwrap();
    fs::write(pack.join("loop.opus"), b"opus").unwrap();
    pack
}

fn settings() -> Settings {
    let mut s = Settings::default();
    s.workers = 2;
    s
}

fn plan_for(pack: &Path, out: &Path) -> Plan {
    build_plan(&[pack.to_path_buf()], Some(out), &settings()).unwrap()
}

#[test]
fn converts_copies_and_reports_events() {
    let tmp = tempfile::tempdir().unwrap();
    let pack = make_pack(tmp.path());
    let out = tmp.path().join("out");
    fs::create_dir_all(&out).unwrap();
    let plan = plan_for(&pack, &out);
    let events = Mutex::new(Vec::new());
    let (summary, _) = run(&plan, &settings(), &AtomicBool::new(false), None, |e| events.lock().unwrap().push(e)).unwrap();

    assert_eq!((summary.converted, summary.copied, summary.failed, summary.skipped), (2, 1, 1, 0));
    assert!(out.join("Pack/Drums/kick.wav").exists());
    assert!(out.join("Pack/artwork.jpg").exists());
    assert!(!out.join("Pack/loop.opus").exists());
    let order: Vec<String> = summary.results.iter().map(|r| r.source.file_name().unwrap().to_string_lossy().into_owned()).collect();
    assert_eq!(order, ["kick.wav", "snare.wav", "artwork.jpg", "loop.opus"]);

    let events = events.into_inner().unwrap();
    assert!(matches!(events.first(), Some(Event::Started { total: 4 })));
    assert!(matches!(events.last(), Some(Event::Finished { .. })));
    let done: Vec<usize> = events.iter().filter_map(|e| match e { Event::FileDone { done, .. } => Some(*done), _ => None }).collect();
    assert_eq!(done.len(), 4);
    assert_eq!(done.iter().max(), Some(&4));
}

#[test]
fn rerun_skips_existing_outputs() {
    let tmp = tempfile::tempdir().unwrap();
    let pack = make_pack(tmp.path());
    let out = tmp.path().join("out");
    let plan = plan_for(&pack, &out);
    run(&plan, &settings(), &AtomicBool::new(false), None, |_| {}).unwrap();
    let plan = plan_for(&pack, &out);
    let (summary, manifest) = run(&plan, &settings(), &AtomicBool::new(false), None, |_| {}).unwrap();
    assert_eq!((summary.skipped, summary.converted, summary.failed), (3, 0, 1));
    assert!(manifest.files.is_empty());
}

#[test]
fn cancelled_run_writes_nothing() {
    let tmp = tempfile::tempdir().unwrap();
    let pack = make_pack(tmp.path());
    let out = tmp.path().join("out");
    let plan = plan_for(&pack, &out);
    let (summary, manifest) = run(&plan, &settings(), &AtomicBool::new(true), None, |_| {}).unwrap();
    assert_eq!(summary.cancelled, 4);
    assert!(summary.results.iter().all(|r| r.status == FileStatus::Cancelled));
    assert!(manifest.files.is_empty());
    assert!(!out.join("Pack/Drums/kick.wav").exists());
}

#[test]
fn manifest_allows_full_undo() {
    let tmp = tempfile::tempdir().unwrap();
    let pack = make_pack(tmp.path());
    let out = tmp.path().join("out");
    fs::create_dir_all(&out).unwrap();
    let plan = plan_for(&pack, &out);
    let (_, manifest) = run(&plan, &settings(), &AtomicBool::new(false), None, |_| {}).unwrap();
    assert_eq!(manifest.files.len(), 3);
    assert!(manifest.dirs.contains(&out.join("Pack")));
    assert!(manifest.dirs.contains(&out.join("Pack").join("Drums")));
    undo(&manifest, true);
    assert!(!out.join("Pack").exists());
    assert!(out.exists());
}

#[test]
fn json_log_is_written_and_recorded() {
    let tmp = tempfile::tempdir().unwrap();
    let pack = make_pack(tmp.path());
    let out = tmp.path().join("out");
    let plan = plan_for(&pack, &out);
    let mut s = settings();
    s.write_json_log = true;
    let (_, manifest) = run(&plan, &s, &AtomicBool::new(false), None, |_| {}).unwrap();
    let log = out.join("Pack").join(LOG_FILE_NAME);
    let parsed: serde_json::Value = serde_json::from_str(&fs::read_to_string(&log).unwrap()).unwrap();
    assert_eq!(parsed.as_array().unwrap().len(), 4);
    assert_eq!(parsed[3]["status"]["kind"], "failed");
    assert!(manifest.files.iter().any(|f| f.path == log));
}

#[test]
fn broken_file_does_not_stop_the_batch() {
    let tmp = tempfile::tempdir().unwrap();
    let pack = tmp.path().join("Pack");
    fs::create_dir_all(&pack).unwrap();
    fs::write(pack.join("a_broken.wav"), b"not audio").unwrap();
    write_i16_wav(&pack.join("b_good.wav"), 48_000, 1, &[1, 2, 3]);
    let out = tmp.path().join("out");
    let plan = plan_for(&pack, &out);
    let (summary, _) = run(&plan, &settings(), &AtomicBool::new(false), None, |_| {}).unwrap();
    assert_eq!((summary.failed, summary.converted), (1, 1));
    assert!(matches!(&summary.results[0].status, FileStatus::Failed { reason } if !reason.is_empty()));
}

#[test]
fn manifest_is_persisted_while_running() {
    // Final review Important 3: if the app dies mid-run, undo must know what this run created.
    let tmp = tempfile::tempdir().unwrap();
    let pack = make_pack(tmp.path());
    let out = tmp.path().join("out");
    let manifest_file = tmp.path().join("data").join("last_session.json");
    let plan = plan_for(&pack, &out);
    let successes = Mutex::new(0usize);
    let checks = Mutex::new(Vec::new());
    run(&plan, &settings(), &AtomicBool::new(false), Some(&manifest_file), |e| {
        if let Event::FileDone { result, .. } = e {
            let mut ok = successes.lock().unwrap();
            if matches!(result.status, FileStatus::Converted | FileStatus::Copied) {
                *ok += 1;
            }
            let on_disk = Manifest::load_from(&manifest_file).unwrap().map_or(0, |m| m.files.len());
            checks.lock().unwrap().push((*ok, on_disk));
        }
    })
    .unwrap();
    let checks = checks.into_inner().unwrap();
    assert_eq!(checks.len(), 4);
    // Another worker may record its file before this event is emitted, so disk can be ahead.
    assert!(checks.iter().all(|(ok, on_disk)| on_disk >= ok), "{checks:?}");
    let m = Manifest::load_from(&manifest_file).unwrap().unwrap();
    assert!(m.dirs.contains(&out.join("Pack")));
}

#[test]
fn run_that_creates_nothing_keeps_previous_manifest() {
    let tmp = tempfile::tempdir().unwrap();
    let pack = make_pack(tmp.path());
    let out = tmp.path().join("out");
    let manifest_file = tmp.path().join("data").join("last_session.json");
    run(&plan_for(&pack, &out), &settings(), &AtomicBool::new(false), Some(&manifest_file), |_| {}).unwrap();
    run(&plan_for(&pack, &out), &settings(), &AtomicBool::new(false), Some(&manifest_file), |_| {}).unwrap();
    assert_eq!(Manifest::load_from(&manifest_file).unwrap().unwrap().files.len(), 3);
}

#[test]
fn json_log_failure_is_not_fatal() {
    let tmp = tempfile::tempdir().unwrap();
    let pack = make_pack(tmp.path());
    let out = tmp.path().join("out");
    fs::create_dir_all(out.join("Pack").join(LOG_FILE_NAME)).unwrap();
    let manifest_file = tmp.path().join("data").join("last_session.json");
    let mut s = settings();
    s.write_json_log = true;
    let (summary, _) = run(&plan_for(&pack, &out), &s, &AtomicBool::new(false), Some(&manifest_file), |_| {}).unwrap();
    assert_eq!(summary.converted, 2);
    assert_eq!(Manifest::load_from(&manifest_file).unwrap().unwrap().files.len(), 3);
}

#[test]
fn json_log_includes_analysis() {
    let tmp = tempfile::tempdir().unwrap();
    let pack = make_pack(tmp.path());
    let out = tmp.path().join("out");
    let mut s = settings();
    s.detect_key_bpm = true;
    s.write_json_log = true;
    let plan = build_plan(&[pack], Some(&out), &s).unwrap();
    run(&plan, &s, &AtomicBool::new(false), None, |_| {}).unwrap();
    let log = fs::read_to_string(out.join("Pack").join(LOG_FILE_NAME)).unwrap();
    let compact: String = log.chars().filter(|c| !c.is_whitespace()).collect();
    assert!(compact.contains("\"analysis\""), "{log}");
    let version = format!("\"analyzerVersion\":{}", sp404_core::analyze::ANALYZER_VERSION);
    assert!(compact.contains(&version), "{log}");
}

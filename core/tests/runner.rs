mod common;

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::Mutex;

use common::*;
use sp404_core::config::Settings;
use sp404_core::runner::{run, Event, FileStatus, LOG_FILE_NAME};
use sp404_core::scan::{build_plan, Plan};
use sp404_core::undo::undo;

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
    let (summary, _) = run(&plan, &settings(), &AtomicBool::new(false), |e| events.lock().unwrap().push(e)).unwrap();

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
    run(&plan, &settings(), &AtomicBool::new(false), |_| {}).unwrap();
    let plan = plan_for(&pack, &out);
    let (summary, manifest) = run(&plan, &settings(), &AtomicBool::new(false), |_| {}).unwrap();
    assert_eq!((summary.skipped, summary.converted, summary.failed), (3, 0, 1));
    assert!(manifest.files.is_empty());
}

#[test]
fn cancelled_run_writes_nothing() {
    let tmp = tempfile::tempdir().unwrap();
    let pack = make_pack(tmp.path());
    let out = tmp.path().join("out");
    let plan = plan_for(&pack, &out);
    let (summary, manifest) = run(&plan, &settings(), &AtomicBool::new(true), |_| {}).unwrap();
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
    let (_, manifest) = run(&plan, &settings(), &AtomicBool::new(false), |_| {}).unwrap();
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
    let (_, manifest) = run(&plan, &s, &AtomicBool::new(false), |_| {}).unwrap();
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
    let (summary, _) = run(&plan, &settings(), &AtomicBool::new(false), |_| {}).unwrap();
    assert_eq!((summary.failed, summary.converted), (1, 1));
    assert!(matches!(&summary.results[0].status, FileStatus::Failed { reason } if !reason.is_empty()));
}

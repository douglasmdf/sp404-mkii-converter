mod common;

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Mutex;

use common::*;
use sp404_core::analyze::AnalysisCache;
use sp404_core::config::{SaveMode, Settings};
use sp404_core::scan::{build_plan, inspect_inputs, inspect_pack, plan_with, JobKind, Plan, PlanProgress, LOOSE_FOLDER_NAME};
use sp404_core::tags::SampleKind;
use sp404_core::CoreError;

fn touch(root: &Path, rel: &str) {
    let path = root.join(rel);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, b"x").unwrap();
}

fn rel_dests(plan: &Plan) -> Vec<String> {
    let pack = &plan.packs[0];
    pack.jobs
        .iter()
        .map(|j| j.dest.strip_prefix(&pack.dest_root).unwrap().to_string_lossy().replace('\\', "/"))
        .collect()
}

fn setup() -> (tempfile::TempDir, PathBuf, PathBuf) {
    let tmp = tempfile::tempdir().unwrap();
    let pack = tmp.path().join("Pack");
    let out = tmp.path().join("out");
    fs::create_dir_all(&pack).unwrap();
    fs::create_dir_all(&out).unwrap();
    (tmp, pack, out)
}

#[test]
fn preserves_structure_and_converts_extension() {
    let (_tmp, pack, out) = setup();
    touch(&pack, "Breaks/Funky Break.mp3");
    touch(&pack, "One Shots/kick.aif");
    touch(&pack, "artwork.jpg");
    let plan = build_plan(&[pack.clone()], Some(&out), &Settings::default()).unwrap();
    assert_eq!(plan.packs[0].dest_root, out.join("Pack"));
    assert_eq!(rel_dests(&plan), ["Breaks/Funky_Break.wav", "One Shots/kick.wav", "artwork.jpg"]);
    let kinds: Vec<JobKind> = plan.packs[0].jobs.iter().map(|j| j.kind).collect();
    assert_eq!(kinds, [JobKind::Convert, JobKind::Convert, JobKind::Copy]);
}

#[test]
fn save_modes_choose_destination() {
    let (tmp, pack, out) = setup();
    touch(&pack, "kick.wav");

    let ask = Settings::default();
    assert!(matches!(build_plan(&[pack.clone()], None, &ask), Err(CoreError::NoOutputDir)));

    let mut fixed = Settings::default();
    fixed.save_mode = SaveMode::Fixed;
    assert!(matches!(build_plan(&[pack.clone()], None, &fixed), Err(CoreError::NoOutputDir)));
    fixed.fixed_directory = Some(out.clone());
    let plan = build_plan(&[pack.clone()], None, &fixed).unwrap();
    assert_eq!(plan.packs[0].dest_root, out.join("Pack"));

    let mut next = Settings::default();
    next.save_mode = SaveMode::NextToSource;
    let plan = build_plan(&[pack.clone()], None, &next).unwrap();
    assert_eq!(plan.packs[0].dest_root, tmp.path().join("Pack [SP-404]"));
}

#[test]
fn sanitize_folder_names_applies_to_pack_and_subfolders() {
    let tmp = tempfile::tempdir().unwrap();
    let pack = tmp.path().join("My Pack");
    let out = tmp.path().join("out");
    touch(&pack, "Lo Fi/kick.wav");
    let mut s = Settings::default();
    s.sanitize_folder_names = true;
    let plan = build_plan(&[pack], Some(&out), &s).unwrap();
    assert_eq!(plan.packs[0].dest_root, out.join("My_Pack"));
    assert_eq!(rel_dests(&plan), ["Lo_Fi/kick.wav"]);
}

#[test]
fn flatten_moves_everything_to_pack_root() {
    let (_tmp, pack, out) = setup();
    touch(&pack, "Breaks/Funk/kick.wav");
    touch(&pack, "Breaks/readme.txt");
    let mut s = Settings::default();
    s.flatten_folders = true;
    let plan = build_plan(&[pack], Some(&out), &s).unwrap();
    assert_eq!(rel_dests(&plan), ["Breaks_Funk_kick.wav", "Breaks_readme.txt"]);
}

#[test]
fn max_length_truncates_converted_names_only() {
    let (_tmp, pack, out) = setup();
    touch(&pack, "a_very_long_name.wav");
    touch(&pack, "a_very_long_readme.txt");
    let mut s = Settings::default();
    s.max_filename_length = Some(8);
    let plan = build_plan(&[pack], Some(&out), &s).unwrap();
    assert_eq!(rel_dests(&plan), ["a_very_l.wav", "a_very_long_readme.txt"]);
}

#[test]
fn same_stem_different_extensions_all_survive() {
    // Review Focus 1
    let (_tmp, pack, out) = setup();
    touch(&pack, "snare.wav");
    touch(&pack, "snare.aif");
    touch(&pack, "Snare.mp3");
    let plan = build_plan(&[pack], Some(&out), &Settings::default()).unwrap();
    assert_eq!(rel_dests(&plan), ["Snare.wav", "snare_2.wav", "snare_3.wav"]);
}

#[test]
fn output_inside_source_is_rejected() {
    // Review Focus 2
    let (tmp, pack, _out) = setup();
    touch(&pack, "kick.wav");
    let same = build_plan(&[pack.clone()], Some(tmp.path()), &Settings::default());
    assert!(matches!(same, Err(CoreError::OutputInsideSource { .. })));
    let inside = build_plan(&[pack.clone()], Some(&pack), &Settings::default());
    assert!(matches!(inside, Err(CoreError::OutputInsideSource { .. })));
}

#[test]
fn system_junk_is_ignored() {
    // Review Focus 3
    let (_tmp, pack, out) = setup();
    touch(&pack, "kick.wav");
    touch(&pack, "._kick.wav");
    touch(&pack, ".DS_Store");
    touch(&pack, "__MACOSX/kick.wav");
    touch(&pack, "Thumbs.db");
    touch(&pack, "Drums/desktop.ini");
    let plan = build_plan(&[pack], Some(&out), &Settings::default()).unwrap();
    assert_eq!(rel_dests(&plan), ["kick.wav"]);
}

#[test]
fn unsupported_audio_is_flagged_and_extensions_ignore_case() {
    let (_tmp, pack, out) = setup();
    touch(&pack, "loop.opus");
    touch(&pack, "old.WMA");
    touch(&pack, "bass.FLAC");
    let plan = build_plan(&[pack], Some(&out), &Settings::default()).unwrap();
    let kinds: Vec<(String, JobKind)> = plan.packs[0]
        .jobs
        .iter()
        .map(|j| (j.source.file_name().unwrap().to_string_lossy().into_owned(), j.kind))
        .collect();
    assert_eq!(
        kinds,
        [
            ("bass.FLAC".to_string(), JobKind::Convert),
            ("loop.opus".to_string(), JobKind::Unsupported),
            ("old.WMA".to_string(), JobKind::Unsupported),
        ]
    );
}

#[test]
fn existing_destination_is_marked() {
    let (_tmp, pack, out) = setup();
    touch(&pack, "kick.wav");
    touch(&out, "Pack/kick.wav");
    let plan = build_plan(&[pack], Some(&out), &Settings::default()).unwrap();
    assert!(plan.packs[0].jobs[0].already_exists);
}

#[test]
fn packs_with_same_name_get_distinct_roots() {
    let tmp = tempfile::tempdir().unwrap();
    let a = tmp.path().join("a").join("Drums");
    let b = tmp.path().join("b").join("Drums");
    touch(&a, "kick.wav");
    touch(&b, "kick.wav");
    let out = tmp.path().join("out");
    let plan = build_plan(&[a, b], Some(&out), &Settings::default()).unwrap();
    assert_eq!(plan.packs[0].dest_root, out.join("Drums"));
    assert_eq!(plan.packs[1].dest_root, out.join("Drums_2"));
}

#[test]
fn inspect_counts_files_by_kind() {
    let (_tmp, pack, _out) = setup();
    touch(&pack, "a.wav");
    touch(&pack, "Sub/b.mp3");
    touch(&pack, "c.opus");
    touch(&pack, "art.png");
    touch(&pack, ".DS_Store");
    let s = inspect_pack(&pack).unwrap();
    assert_eq!(s.name, "Pack");
    assert_eq!((s.audio_files, s.other_files, s.unsupported_files), (2, 1, 1));
}

#[test]
fn output_inside_another_pack_of_the_batch_is_rejected() {
    // Final review Important 1, scenario A: Downloads/Drums + Samples/Drums saved into Samples.
    let tmp = tempfile::tempdir().unwrap();
    let downloads = tmp.path().join("Downloads").join("Drums");
    let samples = tmp.path().join("Samples").join("Drums");
    touch(&downloads, "kick.wav");
    touch(&samples, "kick.wav");
    let result = build_plan(&[downloads, samples], Some(&tmp.path().join("Samples")), &Settings::default());
    assert!(matches!(result, Err(CoreError::OutputInsideSource { .. })));
}

#[test]
fn output_containing_the_source_is_rejected() {
    // Final review Important 1, scenario B: Windows "Extract All" makes DL/808 Pack/808 Pack.
    let tmp = tempfile::tempdir().unwrap();
    let inner = tmp.path().join("DL").join("808 Pack").join("808 Pack");
    touch(&inner, "Kicks/k.wav");
    let result = build_plan(&[inner], Some(&tmp.path().join("DL")), &Settings::default());
    assert!(matches!(result, Err(CoreError::OutputInsideSource { .. })));
}

#[test]
fn nested_packs_next_to_source_are_rejected() {
    // Final review Important 1, scenario C.
    let tmp = tempfile::tempdir().unwrap();
    let drums = tmp.path().join("Drums");
    let kicks = drums.join("Kicks");
    touch(&kicks, "k.wav");
    let mut s = Settings::default();
    s.save_mode = SaveMode::NextToSource;
    assert!(matches!(build_plan(&[drums, kicks], None, &s), Err(CoreError::OutputInsideSource { .. })));
}

fn loose_dests(plan: &Plan) -> Vec<String> {
    let pack = plan.packs.last().unwrap();
    pack.jobs
        .iter()
        .map(|j| j.dest.strip_prefix(&pack.dest_root).unwrap().to_string_lossy().replace('\\', "/"))
        .collect()
}

#[test]
fn loose_files_go_to_samples_folder() {
    let tmp = tempfile::tempdir().unwrap();
    touch(tmp.path(), "a/Kick.wav");
    touch(tmp.path(), "b/Loop.mp3");
    let out = tmp.path().join("out");
    let files = [tmp.path().join("a/Kick.wav"), tmp.path().join("b/Loop.mp3")];
    let plan = build_plan(&files, Some(&out), &Settings::default()).unwrap();
    assert_eq!(plan.packs.len(), 1);
    assert!(plan.packs[0].loose);
    assert_eq!(plan.packs[0].name, LOOSE_FOLDER_NAME);
    assert_eq!(plan.packs[0].dest_root, out.join("Samples [SP-404]"));
    assert_eq!(loose_dests(&plan), ["Kick.wav", "Loop.wav"]);
    assert_eq!(plan.packs[0].jobs[0].source, files[0]);
}

#[test]
fn loose_files_ignore_save_mode() {
    let tmp = tempfile::tempdir().unwrap();
    touch(tmp.path(), "Pack/snare.wav");
    touch(tmp.path(), "kick.wav");
    let out = tmp.path().join("out");
    let other = tmp.path().join("other");
    let mut s = Settings::default();
    s.save_mode = SaveMode::Fixed;
    s.fixed_directory = Some(other.clone());
    let inputs = [tmp.path().join("kick.wav"), tmp.path().join("Pack")];
    let plan = build_plan(&inputs, Some(&out), &s).unwrap();
    assert_eq!(plan.packs.len(), 2);
    assert!(!plan.packs[0].loose);
    assert_eq!(plan.packs[0].dest_root, other.join("Pack"));
    assert!(plan.packs[1].loose);
    assert_eq!(plan.packs[1].dest_root, out.join("Samples [SP-404]"));
}

#[test]
fn loose_files_need_output_dir() {
    let tmp = tempfile::tempdir().unwrap();
    touch(tmp.path(), "kick.wav");
    let mut s = Settings::default();
    s.save_mode = SaveMode::NextToSource;
    let result = build_plan(&[tmp.path().join("kick.wav")], None, &s);
    assert!(matches!(result, Err(CoreError::NoOutputDir)));
}

#[test]
fn loose_output_may_be_the_source_folder() {
    let tmp = tempfile::tempdir().unwrap();
    touch(tmp.path(), "src/kick.wav");
    let src = tmp.path().join("src");
    let plan = build_plan(&[src.join("kick.wav")], Some(&src), &Settings::default()).unwrap();
    assert_eq!(plan.packs[0].jobs[0].dest, src.join("Samples [SP-404]").join("kick.wav"));
}

#[test]
fn loose_file_inside_output_is_rejected() {
    let tmp = tempfile::tempdir().unwrap();
    touch(tmp.path(), "out/Samples [SP-404]/kick.wav");
    let out = tmp.path().join("out");
    let result = build_plan(&[out.join("Samples [SP-404]/kick.wav")], Some(&out), &Settings::default());
    assert!(matches!(result, Err(CoreError::OutputInsideSource { .. })));
}

#[test]
fn loose_duplicate_names_are_numbered() {
    let tmp = tempfile::tempdir().unwrap();
    touch(tmp.path(), "a/kick.wav");
    touch(tmp.path(), "b/Kick.aif");
    let out = tmp.path().join("out");
    let files = [tmp.path().join("a/kick.wav"), tmp.path().join("b/Kick.aif")];
    let plan = build_plan(&files, Some(&out), &Settings::default()).unwrap();
    assert_eq!(loose_dests(&plan), ["kick.wav", "Kick_2.wav"]);
}

#[test]
fn loose_non_audio_is_rejected() {
    let tmp = tempfile::tempdir().unwrap();
    touch(tmp.path(), "notes.pdf");
    let out = tmp.path().join("out");
    let pdf = build_plan(&[tmp.path().join("notes.pdf")], Some(&out), &Settings::default());
    assert!(matches!(pdf, Err(CoreError::NotAudioFile(_))));
    let missing = build_plan(&[tmp.path().join("missing.wav")], Some(&out), &Settings::default());
    assert!(matches!(missing, Err(CoreError::NotFound(_))));
}

#[test]
fn inspect_inputs_sorts_paths() {
    let tmp = tempfile::tempdir().unwrap();
    touch(tmp.path(), "Pack/a.wav");
    touch(tmp.path(), "one.wav");
    touch(tmp.path(), "two.opus");
    touch(tmp.path(), "doc.pdf");
    let inputs = [
        tmp.path().join("Pack"),
        tmp.path().join("one.wav"),
        tmp.path().join("two.opus"),
        tmp.path().join("doc.pdf"),
    ];
    let summary = inspect_inputs(&inputs).unwrap();
    assert_eq!(summary.packs.len(), 1);
    let loose = summary.loose.unwrap();
    assert_eq!(loose.name, LOOSE_FOLDER_NAME);
    assert_eq!((loose.audio_files, loose.unsupported_files), (1, 1));
    assert_eq!(summary.rejected, [tmp.path().join("doc.pdf")]);
    assert!(matches!(
        inspect_inputs(&[tmp.path().join("nope")]),
        Err(CoreError::NotFound(_))
    ));
}

#[test]
fn folders_only_plan_is_unchanged() {
    let (_tmp, pack, out) = setup();
    touch(&pack, "Breaks/Funky Break.mp3");
    touch(&pack, "One Shots/kick.aif");
    touch(&pack, "artwork.jpg");
    let plan = build_plan(&[pack], Some(&out), &Settings::default()).unwrap();
    assert!(plan.packs.iter().all(|p| !p.loose));
    assert_eq!(rel_dests(&plan), ["Breaks/Funky_Break.wav", "One Shots/kick.wav", "artwork.jpg"]);
}

const RATE: u32 = 44_100;

/// Fixed-seed noise in [-1, 1).
fn noise(seed: &mut u32) -> f32 {
    *seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
    (*seed >> 8) as f32 / (1u32 << 23) as f32 - 1.0
}

fn to_i16(samples: &[f32]) -> Vec<i16> {
    samples.iter().map(|s| (s * 32767.0).round() as i16).collect()
}

fn ensure_parent(path: &Path) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
}

/// One 30 ms noise click per beat, cut exactly at `beats` beats.
fn write_clicks(path: &Path, bpm: f32, beats: usize) {
    ensure_parent(path);
    let beat = 60.0 / bpm * RATE as f32;
    let mut out = vec![0f32; (beat * beats as f32).round() as usize];
    let mut seed = 777;
    for b in 0..beats {
        let start = (b as f32 * beat).round() as usize;
        for i in 0..(0.03 * RATE as f32) as usize {
            if let Some(slot) = out.get_mut(start + i) {
                *slot = 0.4 * noise(&mut seed) * (-(i as f32 / RATE as f32) / 0.004).exp();
            }
        }
    }
    write_i16_wav(path, RATE, 1, &to_i16(&out));
}

/// A decaying burst of noise (no pitch).
fn write_noise(path: &Path, seconds: f32) {
    ensure_parent(path);
    let n = (seconds * RATE as f32) as usize;
    let mut seed = 12_345;
    let out: Vec<f32> = (0..n).map(|i| 0.5 * noise(&mut seed) * (-(i as f32 / RATE as f32) / (seconds / 5.0)).exp()).collect();
    write_i16_wav(path, RATE, 1, &to_i16(&out));
}

fn write_sine(path: &Path, freq: f32, seconds: f32) {
    ensure_parent(path);
    let fmt = WavFormat { sample_rate: RATE, channels: 1, bits: 16, float: false };
    write_sine_wav(path, &fmt, freq, 0.5, (seconds * RATE as f32) as usize);
}

fn detecting() -> Settings {
    let mut s = Settings::default();
    s.detect_key_bpm = true;
    s.workers = 4;
    s
}

fn plan(pack: &Path, out: &Path, settings: &Settings) -> Plan {
    plan_with(&[pack.to_path_buf()], Some(out), settings, &AnalysisCache::default(), &AtomicBool::new(false), |_| {}).unwrap()
}

/// sub.wav (55 Hz sine: a note A), clicks (90bpm).wav (a drum loop) and kick.wav (noise).
fn detection_pack(pack: &Path) {
    write_sine(&pack.join("One Shots/Bass/sub.wav"), 55.0, 1.0);
    write_clicks(&pack.join("Loops/clicks (90bpm).wav"), 90.0, 16);
    write_noise(&pack.join("Drums/Kick/kick.wav"), 0.2);
}

#[test]
fn detection_off_plan_is_unchanged() {
    let (_tmp, pack, out) = setup();
    touch(&pack, "Breaks/Funky Break.mp3");
    touch(&pack, "One Shots/kick.aif");
    touch(&pack, "artwork.jpg");
    let plan = build_plan(&[pack.clone()], Some(&out), &Settings::default()).unwrap();
    let job = |src: &str, dest: &str, kind: &str| {
        serde_json::json!({ "source": pack.join(src), "dest": out.join("Pack").join(dest), "kind": kind, "alreadyExists": false })
    };
    let expected = serde_json::json!({ "packs": [{
        "name": "Pack",
        "sourceRoot": pack,
        "destRoot": out.join("Pack"),
        "jobs": [
            job("Breaks/Funky Break.mp3", "Breaks/Funky_Break.wav", "convert"),
            job("One Shots/kick.aif", "One Shots/kick.wav", "convert"),
            job("artwork.jpg", "artwork.jpg", "copy"),
        ],
        "loose": false,
    }]});
    assert_eq!(serde_json::to_value(&plan).unwrap(), expected);
}

#[test]
fn detection_prefixes_names() {
    let (_tmp, pack, out) = setup();
    detection_pack(&pack);
    let plan = plan(&pack, &out, &detecting());
    assert_eq!(rel_dests(&plan), ["Drums/Kick/kick.wav", "Loops/90_clicks.wav", "One Shots/Bass/A_sub.wav"]);
    let analyses: Vec<_> = plan.packs[0].jobs.iter().map(|j| j.analysis.clone().expect("analysed")).collect();
    let kinds: Vec<SampleKind> = analyses.iter().map(|a| a.kind).collect();
    assert_eq!(kinds, [SampleKind::Unpitched, SampleKind::DrumLoop, SampleKind::Note]);
    assert!(analyses[1].bpm_from_name && !analyses[2].label_from_name);
}

#[test]
fn prefix_survives_max_length_and_collisions() {
    let (_tmp, pack, out) = setup();
    write_clicks(&pack.join("Loops/clicks (90bpm).wav"), 90.0, 16);
    write_clicks(&pack.join("Loops/clicks 90bpm.wav"), 90.0, 16);
    let mut s = detecting();
    s.max_filename_length = Some(6);
    assert_eq!(rel_dests(&plan(&pack, &out, &s)), ["Loops/90_cli.wav", "Loops/90_c_2.wav"]);
}

#[test]
fn flatten_puts_prefix_first() {
    let (_tmp, pack, out) = setup();
    write_clicks(&pack.join("Loops/clicks (90bpm).wav"), 90.0, 16);
    let mut s = detecting();
    s.flatten_folders = true;
    assert_eq!(rel_dests(&plan(&pack, &out, &s)), ["90_Loops_clicks.wav"]);
}

#[test]
fn unanalyzable_files_get_no_prefix() {
    let (_tmp, pack, out) = setup();
    write_i16_wav(&pack.join("empty.wav"), RATE, 1, &[]);
    write_i16_wav(&pack.join("silent.wav"), RATE, 1, &vec![0; RATE as usize]);
    fs::write(pack.join("broken.wav"), b"definitely not a wav file").unwrap();
    let plan = plan(&pack, &out, &detecting());
    assert_eq!(rel_dests(&plan), ["broken.wav", "empty.wav", "silent.wav"]);
    let analysis = |i: usize| plan.packs[0].jobs[i].analysis.clone().expect("analysed");
    for i in 0..3 {
        assert_eq!(analysis(i).prefix, None);
    }
    assert!(analysis(0).error.is_some() && analysis(1).error.is_some());
}

#[test]
fn detection_names_are_deterministic_across_workers_and_runs() {
    let (_tmp, pack, out) = setup();
    detection_pack(&pack);
    write_sine(&pack.join("Keys/tone.wav"), 261.63, 1.5);
    write_clicks(&pack.join("Loops/fast.wav"), 128.0, 32);
    let mut one = detecting();
    one.workers = 1;
    let mut eight = detecting();
    eight.workers = 8;
    let first = plan(&pack, &out, &one);
    assert_eq!(plan(&pack, &out, &eight), first);
    assert_eq!(plan(&pack, &out, &eight), first);
}

#[test]
fn cancel_stops_analysis() {
    let (_tmp, pack, out) = setup();
    detection_pack(&pack);
    let out = out.join("new");
    let result = plan_with(&[pack.clone()], Some(&out), &detecting(), &AnalysisCache::default(), &AtomicBool::new(true), |_| {});
    assert!(matches!(result, Err(CoreError::Cancelled)), "{result:?}");
    assert!(!out.exists());

    let calls = AtomicUsize::new(0);
    let seen = Mutex::new(Vec::new());
    plan_with(&[pack], Some(&out), &detecting(), &AnalysisCache::default(), &AtomicBool::new(false), |p: PlanProgress| {
        calls.fetch_add(1, Ordering::SeqCst);
        seen.lock().unwrap().push(p);
    })
    .unwrap();
    assert_eq!(calls.load(Ordering::SeqCst), 3);
    assert!(seen.lock().unwrap().contains(&PlanProgress::Analyzing { done: 3, total: 3 }));
}

#[test]
fn cache_avoids_second_analysis() {
    let (_tmp, pack, out) = setup();
    detection_pack(&pack);
    let cache = AnalysisCache::default();
    let run = || plan_with(&[pack.clone()], Some(&out), &detecting(), &cache, &AtomicBool::new(false), |_| {}).unwrap();
    let first = run();
    assert_eq!((cache.len(), cache.hits()), (3, 0));
    assert_eq!(run(), first);
    assert_eq!((cache.len(), cache.hits()), (3, 3));
}

#[test]
fn progress_event_json() {
    let json = serde_json::to_string(&PlanProgress::Analyzing { done: 1, total: 2 }).unwrap();
    assert_eq!(json, r#"{"event":"analyzing","done":1,"total":2}"#);
}

#[test]
fn name_that_is_only_bpm_becomes_the_prefix() {
    let (_tmp, pack, out) = setup();
    write_clicks(&pack.join("Loops/90bpm.wav"), 90.0, 16);
    assert_eq!(rel_dests(&plan(&pack, &out, &detecting())), ["Loops/90.wav"]);
    let mut flat = detecting();
    flat.flatten_folders = true;
    flat.sanitize_filenames = false;
    assert_eq!(rel_dests(&plan(&pack, &out, &flat)), ["90_Loops.wav"]);
}

#[test]
fn cache_keeps_failures_too() {
    let (_tmp, pack, out) = setup();
    fs::write(pack.join("broken.wav"), b"definitely not a wav file").unwrap();
    write_i16_wav(&pack.join("silent.wav"), RATE, 1, &vec![0; RATE as usize]);
    let cache = AnalysisCache::default();
    let run = || plan_with(&[pack.clone()], Some(&out), &detecting(), &cache, &AtomicBool::new(false), |_| {}).unwrap();
    let first = run();
    assert_eq!(cache.hits(), 0);
    assert_eq!(run(), first);
    assert_eq!(cache.hits(), 2);
    assert!(first.packs[0].jobs.iter().all(|j| j.analysis.as_ref().unwrap().error.is_some()));
}

#[test]
fn limit_shorter_than_prefix_keeps_it_whole() {
    let (_tmp, pack, out) = setup();
    write_clicks(&pack.join("Loops/Loop 140bpm.wav"), 140.0, 16);
    write_clicks(&pack.join("Loops/Loop_140bpm.wav"), 140.0, 16);
    let mut s = detecting();
    s.max_filename_length = Some(2);
    assert_eq!(rel_dests(&plan(&pack, &out, &s)), ["Loops/140.wav", "Loops/140_2.wav"]);
}

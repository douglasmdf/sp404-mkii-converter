use std::fs;
use std::path::{Path, PathBuf};

use sp404_core::config::{SaveMode, Settings};
use sp404_core::scan::{build_plan, inspect_pack, JobKind, Plan};
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
fn file_instead_of_folder_is_rejected() {
    let (_tmp, pack, out) = setup();
    touch(&pack, "kick.wav");
    let file = pack.join("kick.wav");
    assert!(matches!(
        build_plan(&[file], Some(&out), &Settings::default()),
        Err(CoreError::NotADirectory(_))
    ));
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

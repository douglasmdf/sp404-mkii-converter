use std::fs;
use std::path::Path;

use sp404_core::undo::{record, undo, undo_last_session, Manifest, ManifestWriter};

fn make_file(path: &Path, content: &[u8]) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, content).unwrap();
}

#[test]
fn removes_recorded_files() {
    let dir = tempfile::tempdir().unwrap();
    let a = dir.path().join("Pack/a.wav");
    let b = dir.path().join("Pack/b.wav");
    make_file(&a, b"aaa");
    make_file(&b, b"bbb");
    let manifest = Manifest { files: vec![record(&a).unwrap(), record(&b).unwrap()], dirs: vec![] };
    let report = undo(&manifest, false);
    assert_eq!(report.removed, 2);
    assert!(!a.exists() && !b.exists());
}

#[test]
fn keeps_files_modified_after_conversion() {
    let dir = tempfile::tempdir().unwrap();
    let a = dir.path().join("a.wav");
    make_file(&a, b"original");
    let manifest = Manifest { files: vec![record(&a).unwrap()], dirs: vec![] };
    fs::write(&a, b"edited by the user, longer").unwrap();
    let report = undo(&manifest, false);
    assert_eq!(report.removed, 0);
    assert_eq!(report.kept_modified, vec![a.clone()]);
    assert!(a.exists());
}

#[test]
fn counts_missing_files() {
    let dir = tempfile::tempdir().unwrap();
    let a = dir.path().join("a.wav");
    make_file(&a, b"x");
    let manifest = Manifest { files: vec![record(&a).unwrap()], dirs: vec![] };
    fs::remove_file(&a).unwrap();
    assert_eq!(undo(&manifest, false).missing, 1);
}

#[test]
fn removes_only_empty_created_dirs_deepest_first() {
    let dir = tempfile::tempdir().unwrap();
    let pack = dir.path().join("Pack");
    let drums = pack.join("Drums");
    let loops = pack.join("Loops");
    let kick = drums.join("kick.wav");
    make_file(&kick, b"k");
    fs::create_dir_all(&loops).unwrap();
    make_file(&loops.join("user-added.txt"), b"keep me");
    let manifest = Manifest {
        files: vec![record(&kick).unwrap()],
        dirs: vec![pack.clone(), drums.clone(), loops.clone()],
    };
    let report = undo(&manifest, true);
    assert_eq!(report.removed, 1);
    assert_eq!(report.removed_dirs, 1);
    assert!(!drums.exists());
    assert!(loops.exists() && pack.exists());
}

#[test]
fn manifest_roundtrips_and_missing_manifest_is_none() {
    let dir = tempfile::tempdir().unwrap();
    let a = dir.path().join("a.wav");
    make_file(&a, b"x");
    let manifest = Manifest { files: vec![record(&a).unwrap()], dirs: vec![dir.path().join("d")] };
    let path = dir.path().join("data").join("last_session.json");
    manifest.save_to(&path).unwrap();
    assert_eq!(Manifest::load_from(&path).unwrap(), Some(manifest));
    assert_eq!(Manifest::load_from(&dir.path().join("none.json")).unwrap(), None);
}

#[test]
fn undo_last_session_consumes_the_manifest() {
    let dir = tempfile::tempdir().unwrap();
    let a = dir.path().join("a.wav");
    make_file(&a, b"x");
    let path = dir.path().join("last_session.json");
    Manifest { files: vec![record(&a).unwrap()], dirs: vec![] }.save_to(&path).unwrap();
    let report = undo_last_session(&path, false).unwrap().unwrap();
    assert_eq!(report.removed, 1);
    assert!(!path.exists());
    assert!(undo_last_session(&path, false).unwrap().is_none());
}

#[test]
fn writer_keeps_old_session_until_first_record_then_appends() {
    let dir = tempfile::tempdir().unwrap();
    let old = dir.path().join("old.wav");
    let a = dir.path().join("Pack/a.wav");
    make_file(&old, b"old");
    make_file(&a, b"new");
    let path = dir.path().join("data").join("last_session.json");
    Manifest { files: vec![record(&old).unwrap()], dirs: vec![] }.save_to(&path).unwrap();

    let writer = ManifestWriter::new(&path);
    assert_eq!(Manifest::load_from(&path).unwrap().unwrap().files[0].path, old);

    writer.record_dir(&dir.path().join("Pack")).unwrap();
    writer.record_file(&a).unwrap();
    let m = Manifest::load_from(&path).unwrap().unwrap();
    assert_eq!(m.dirs, vec![dir.path().join("Pack")]);
    assert_eq!(m.files.len(), 1);
    assert_eq!(m.files[0].path, a);
}

#[test]
fn truncated_last_line_is_ignored() {
    // A crash can leave a half-written last line; everything before it must still be undoable.
    let dir = tempfile::tempdir().unwrap();
    let a = dir.path().join("a.wav");
    make_file(&a, b"x");
    let path = dir.path().join("last_session.json");
    Manifest { files: vec![record(&a).unwrap()], dirs: vec![] }.save_to(&path).unwrap();
    let mut text = fs::read_to_string(&path).unwrap();
    text.push_str("{\"type\":\"file\",\"pa");
    fs::write(&path, text).unwrap();
    assert_eq!(Manifest::load_from(&path).unwrap().unwrap().files.len(), 1);
}

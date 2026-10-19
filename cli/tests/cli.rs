use std::fs;
use std::path::Path;
use std::process::Command;

fn sp404(data_dir: &Path) -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_sp404"));
    cmd.env("SP404_DATA_DIR", data_dir);
    cmd
}

fn write_wav(path: &Path) {
    let spec = hound::WavSpec { channels: 2, sample_rate: 44_100, bits_per_sample: 16, sample_format: hound::SampleFormat::Int };
    let mut w = hound::WavWriter::create(path, spec).unwrap();
    for i in 0..882 {
        w.write_sample(((i % 100) * 100) as i16).unwrap();
    }
    w.finalize().unwrap();
}

fn setup() -> (tempfile::TempDir, std::path::PathBuf, std::path::PathBuf, std::path::PathBuf) {
    let tmp = tempfile::tempdir().unwrap();
    let pack = tmp.path().join("Pack");
    fs::create_dir_all(&pack).unwrap();
    write_wav(&pack.join("Kick Água.wav"));
    let out = tmp.path().join("out");
    let data = tmp.path().join("data");
    (tmp, pack, out, data)
}

#[test]
fn dry_run_lists_jobs_and_writes_nothing() {
    let (_tmp, pack, out, data) = setup();
    let output = sp404(&data).args(["convert", "--dry-run", "--out"]).arg(&out).arg(&pack).output().unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Kick_Agua.wav"), "{stdout}");
    assert!(!out.exists());
}

#[test]
fn convert_then_undo_round_trip() {
    let (_tmp, pack, out, data) = setup();
    let status = sp404(&data).args(["convert", "--out"]).arg(&out).arg(&pack).status().unwrap();
    assert!(status.success());
    assert!(out.join("Pack/Kick_Agua.wav").exists());

    let output = sp404(&data).args(["undo", "--remove-empty-dirs"]).output().unwrap();
    assert!(output.status.success());
    assert!(!out.join("Pack").exists());
    let again = sp404(&data).arg("undo").output().unwrap();
    assert!(String::from_utf8_lossy(&again.stdout).contains("Nothing to undo"));
}

#[test]
fn convert_without_output_folder_fails_clearly() {
    let (_tmp, pack, _out, data) = setup();
    let output = sp404(&data).arg("convert").arg(&pack).output().unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("no output folder"));
}

#[test]
fn failed_file_gives_nonzero_exit_but_converts_the_rest() {
    let (_tmp, pack, out, data) = setup();
    fs::write(pack.join("broken.wav"), b"nope").unwrap();
    let status = sp404(&data).args(["convert", "--out"]).arg(&out).arg(&pack).status().unwrap();
    assert_eq!(status.code(), Some(1));
    assert!(out.join("Pack/Kick_Agua.wav").exists());
}

#[test]
fn loose_file_dry_run_uses_samples_folder() {
    let (tmp, _pack, out, data) = setup();
    let file = tmp.path().join("a.wav");
    write_wav(&file);
    let output = sp404(&data).args(["convert", "--dry-run", "--out"]).arg(&out).arg(&file).output().unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Samples [SP-404]"), "{stdout}");
    assert!(stdout.contains("a.wav"), "{stdout}");
}

#[test]
fn loose_file_without_output_folder_fails_clearly() {
    let (tmp, _pack, _out, data) = setup();
    let file = tmp.path().join("a.wav");
    write_wav(&file);
    let output = sp404(&data).arg("convert").arg(&file).output().unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("no output folder"));
}

fn write_sine(path: &Path, hz: f32) {
    let spec = hound::WavSpec { channels: 1, sample_rate: 44_100, bits_per_sample: 16, sample_format: hound::SampleFormat::Int };
    let mut w = hound::WavWriter::create(path, spec).unwrap();
    let total = 44_100;
    for i in 0..total {
        let t = i as f32 / 44_100.0;
        let env = (1.0 - i as f32 / total as f32).max(0.0);
        w.write_sample((f32::sin(2.0 * std::f32::consts::PI * hz * t) * env * 20_000.0) as i16).unwrap();
    }
    w.finalize().unwrap();
}

fn sine_pack() -> (tempfile::TempDir, std::path::PathBuf, std::path::PathBuf, std::path::PathBuf) {
    let tmp = tempfile::tempdir().unwrap();
    let pack = tmp.path().join("Pack");
    fs::create_dir_all(pack.join("One Shots")).unwrap();
    write_sine(&pack.join("One Shots/x.wav"), 220.0);
    let (out, data) = (tmp.path().join("out"), tmp.path().join("data"));
    (tmp, pack, out, data)
}

#[test]
fn analyze_prints_note_and_label() {
    let (tmp, pack, _out, data) = sine_pack();
    let output = sp404(&data).arg("analyze").arg(&pack).output().unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let stdout = String::from_utf8_lossy(&output.stdout);
    let line = stdout.lines().find(|l| l.contains("x.wav")).expect(&stdout);
    let words: Vec<&str> = line.split_whitespace().collect();
    assert!(words.contains(&"note") && words.contains(&"A"), "{stdout}");
    drop(tmp);
}

#[test]
fn analyze_scores_against_an_answer_key_and_writes_a_report() {
    let (tmp, pack, _out, data) = sine_pack();
    let csv = tmp.path().join("key.csv");
    fs::write(&csv, "path,kind,label,bpm\nOne Shots/x.wav,note,A,\n").unwrap();
    let report = tmp.path().join("report.md");
    let output = sp404(&data).arg("analyze").arg(&pack).arg("--labels").arg(&csv).arg("--report").arg(&report).output().unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let text = fs::read_to_string(&report).unwrap();
    assert!(text.contains("Note exact | 1/1"), "{text}");
}

#[test]
fn convert_dry_run_with_detection_prefixes_the_name() {
    let (_tmp, pack, out, data) = sine_pack();
    let output = sp404(&data)
        .args(["convert", "--dry-run", "--detect-key-bpm", "--out"])
        .arg(&out)
        .arg(&pack)
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("A_x.wav"), "{stdout}");
}

#[test]
fn convert_rejects_an_unknown_bpm_range() {
    let (_tmp, pack, out, data) = sine_pack();
    let output = sp404(&data).args(["convert", "--dry-run", "--bpm-range", "1-2", "--out"]).arg(&out).arg(&pack).output().unwrap();
    assert!(!output.status.success());
}

//! Executes a `Plan` in parallel, reporting progress and recording what it created.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use rayon::prelude::*;
use serde::Serialize;

use crate::config::Settings;
use crate::convert::{convert_file, Warning};
use crate::decode::probe_info;
use crate::encode::copy_file;
use crate::error::{io_err, CoreError, Result};
use crate::labeling::Analysis;
use crate::process::TARGET_RATE;
use crate::scan::{Job, JobKind, Plan};
use crate::undo::{self, Manifest, ManifestWriter};

pub const LOG_FILE_NAME: &str = "conversion_results.json";

/// Above this many samples (≈5 min of stereo at 48 kHz) a file is "heavy": decoding,
/// resampling and quantizing it holds several full copies in memory, so heavy files are
/// converted one at a time instead of alongside other workers.
const HEAVY_SAMPLES: u64 = 48_000 * 60 * 5 * 2;
/// Fallback when the header does not give the length (e.g. some MP3s): ≈20 min at 128 kbps.
const HEAVY_UNKNOWN_BYTES: u64 = 20 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum FileStatus {
    Converted,
    Copied,
    Skipped,
    Failed { reason: String },
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileResult {
    pub source: PathBuf,
    pub dest: PathBuf,
    pub status: FileStatus,
    pub warnings: Vec<Warning>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub analysis: Option<Analysis>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Summary {
    pub converted: usize,
    pub copied: usize,
    pub skipped: usize,
    pub failed: usize,
    pub cancelled: usize,
    pub with_warnings: usize,
    pub results: Vec<FileResult>,
}

impl Summary {
    pub fn from_results(results: Vec<FileResult>) -> Summary {
        let mut summary = Summary::default();
        for result in &results {
            match result.status {
                FileStatus::Converted => summary.converted += 1,
                FileStatus::Copied => summary.copied += 1,
                FileStatus::Skipped => summary.skipped += 1,
                FileStatus::Failed { .. } => summary.failed += 1,
                FileStatus::Cancelled => summary.cancelled += 1,
            }
            if !result.warnings.is_empty() {
                summary.with_warnings += 1;
            }
        }
        summary.results = results;
        summary
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "event", rename_all = "camelCase")]
pub enum Event {
    Started { total: usize },
    FileDone { done: usize, total: usize, result: FileResult },
    Finished { summary: Summary },
}

/// Runs every job of the plan. When `manifest_file` is given, what gets created is appended
/// to it as it happens (see `ManifestWriter`), so undo works even if the run is interrupted.
pub fn run<F>(
    plan: &Plan,
    settings: &Settings,
    cancel: &AtomicBool,
    manifest_file: Option<&Path>,
    on_event: F,
) -> Result<(Summary, Manifest)>
where
    F: Fn(Event) + Sync,
{
    let jobs: Vec<&Job> = plan.jobs().collect();
    let total = jobs.len();
    on_event(Event::Started { total });

    let writer = manifest_file.map(ManifestWriter::new);
    let manifest_error: Mutex<Option<CoreError>> = Mutex::new(None);
    let note = |outcome: Result<()>| {
        if let Err(e) = outcome {
            manifest_error.lock().expect("lock").get_or_insert(e);
        }
    };

    let created_dirs = if cancel.load(Ordering::SeqCst) { Vec::new() } else { create_output_dirs(plan)? };
    if let Some(writer) = &writer {
        for dir in &created_dirs {
            note(writer.record_dir(dir));
        }
    }
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(settings.workers.max(1))
        .build()
        .map_err(|e| CoreError::Internal(e.to_string()))?;
    let done = AtomicUsize::new(0);
    let heavy: Vec<bool> = jobs.iter().map(|job| job_is_heavy(job)).collect();
    let results: Vec<FileResult> = execute(&heavy, &pool, |index| {
        let result = run_job(jobs[index], settings, cancel);
        if let (Some(writer), FileStatus::Converted | FileStatus::Copied) = (&writer, &result.status) {
            note(writer.record_file(&result.dest));
        }
        let done_now = done.fetch_add(1, Ordering::SeqCst) + 1;
        on_event(Event::FileDone { done: done_now, total, result: result.clone() });
        result
    });

    let mut manifest = Manifest {
        files: results
            .iter()
            .filter(|r| matches!(r.status, FileStatus::Converted | FileStatus::Copied))
            .filter_map(|r| undo::record(&r.dest))
            .collect(),
        dirs: created_dirs,
    };

    if settings.write_json_log {
        for pack in &plan.packs {
            if !pack.dest_root.is_dir() {
                continue;
            }
            let pack_results: Vec<&FileResult> =
                results.iter().filter(|r| r.dest.starts_with(&pack.dest_root)).collect();
            let log_path = pack.dest_root.join(LOG_FILE_NAME);
            let json = serde_json::to_string_pretty(&pack_results).map_err(|e| CoreError::Internal(e.to_string()))?;
            // The log is optional: failing to write it must not lose the conversion or undo record.
            if fs::write(&log_path, json).is_ok() {
                if let Some(writer) = &writer {
                    note(writer.record_file(&log_path));
                }
                if let Some(entry) = undo::record(&log_path) {
                    manifest.files.push(entry);
                }
            }
        }
    }

    if let Some(error) = manifest_error.into_inner().expect("lock") {
        return Err(error);
    }
    let summary = Summary::from_results(results);
    on_event(Event::Finished { summary: summary.clone() });
    Ok((summary, manifest))
}

pub fn is_heavy(frames: Option<u64>, sample_rate: Option<u32>, channels: Option<usize>, file_bytes: u64) -> bool {
    match (frames, sample_rate, channels) {
        (Some(frames), Some(rate), Some(channels)) if rate > 0 => {
            let at_output = (frames as f64 * TARGET_RATE as f64 / rate as f64) as u64;
            frames.max(at_output) * channels as u64 > HEAVY_SAMPLES
        }
        _ => file_bytes > HEAVY_UNKNOWN_BYTES,
    }
}

fn job_is_heavy(job: &Job) -> bool {
    if job.kind != JobKind::Convert || job.already_exists {
        return false;
    }
    let bytes = fs::metadata(&job.source).map(|m| m.len()).unwrap_or(0);
    match probe_info(&job.source) {
        Ok(info) => is_heavy(info.frames, info.sample_rate, info.channels, bytes),
        Err(_) => is_heavy(None, None, None, bytes),
    }
}

/// Runs `work` for every index: light jobs in parallel on `pool`, then heavy jobs one at a
/// time. Results come back in index order.
fn execute<T, F>(heavy: &[bool], pool: &rayon::ThreadPool, work: F) -> Vec<T>
where
    T: Send,
    F: Fn(usize) -> T + Sync,
{
    let light: Vec<usize> = (0..heavy.len()).filter(|&i| !heavy[i]).collect();
    let mut slots: Vec<Option<T>> = (0..heavy.len()).map(|_| None).collect();
    let light_results: Vec<(usize, T)> = pool.install(|| light.par_iter().map(|&i| (i, work(i))).collect());
    for (index, result) in light_results {
        slots[index] = Some(result);
    }
    for index in (0..heavy.len()).filter(|&i| heavy[i]) {
        slots[index] = Some(work(index));
    }
    slots.into_iter().map(|slot| slot.expect("every job ran")).collect()
}

fn run_job(job: &Job, settings: &Settings, cancel: &AtomicBool) -> FileResult {
    let mut warnings = Vec::new();
    let status = if cancel.load(Ordering::SeqCst) {
        FileStatus::Cancelled
    } else if job.kind == JobKind::Unsupported {
        FileStatus::Failed { reason: CoreError::UnsupportedFormat.to_string() }
    } else if job.dest.exists() {
        FileStatus::Skipped
    } else if job.kind == JobKind::Convert {
        match convert_file(&job.source, &job.dest, settings) {
            Ok(found) => {
                warnings = found;
                FileStatus::Converted
            }
            Err(e) => FileStatus::Failed { reason: e.to_string() },
        }
    } else {
        match copy_file(&job.source, &job.dest) {
            Ok(()) => FileStatus::Copied,
            Err(e) => FileStatus::Failed { reason: e.to_string() },
        }
    };
    FileResult { source: job.source.clone(), dest: job.dest.clone(), status, warnings, analysis: job.analysis.clone() }
}

/// Creates every missing output folder up front and returns the ones it created,
/// deepest first (the order undo needs).
fn create_output_dirs(plan: &Plan) -> Result<Vec<PathBuf>> {
    let mut missing = BTreeSet::new();
    for job in plan.jobs().filter(|j| j.kind != JobKind::Unsupported) {
        let mut dir = job.dest.parent();
        while let Some(d) = dir {
            if d.as_os_str().is_empty() || d.exists() {
                break;
            }
            missing.insert(d.to_path_buf());
            dir = d.parent();
        }
    }
    let mut dirs: Vec<PathBuf> = missing.into_iter().collect();
    dirs.sort_by_key(|d| d.components().count());
    for dir in &dirs {
        fs::create_dir_all(dir).map_err(io_err(dir))?;
    }
    dirs.reverse();
    Ok(dirs)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;

    #[test]
    fn long_or_large_files_are_heavy() {
        let minute = 48_000u64 * 60;
        assert!(!is_heavy(Some(minute), Some(48_000), Some(2), 10_000_000));
        assert!(is_heavy(Some(10 * minute), Some(48_000), Some(2), 0));
        // 10 minutes at 8 kHz is small on disk but large once resampled to 48 kHz.
        assert!(is_heavy(Some(10 * 60 * 8_000), Some(8_000), Some(2), 0));
        assert!(is_heavy(None, None, None, 50 * 1024 * 1024));
        assert!(!is_heavy(None, None, None, 1_000_000));
    }

    #[test]
    fn execute_keeps_order_and_runs_heavy_jobs_one_at_a_time() {
        let heavy = [false, true, false, true, true, false, false, true];
        let pool = rayon::ThreadPoolBuilder::new().num_threads(4).build().unwrap();
        let active_heavy = AtomicUsize::new(0);
        let max_heavy = AtomicUsize::new(0);
        let results = execute(&heavy, &pool, |i| {
            if heavy[i] {
                let now = active_heavy.fetch_add(1, Ordering::SeqCst) + 1;
                max_heavy.fetch_max(now, Ordering::SeqCst);
                std::thread::sleep(std::time::Duration::from_millis(5));
                active_heavy.fetch_sub(1, Ordering::SeqCst);
            }
            i * 10
        });
        assert_eq!(results, vec![0, 10, 20, 30, 40, 50, 60, 70]);
        assert_eq!(max_heavy.load(Ordering::SeqCst), 1);
    }
}

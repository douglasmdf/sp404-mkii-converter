//! Records what a conversion created so it can be undone safely: a file is deleted only
//! if its size and modification time still match what was recorded.
//!
//! On disk the manifest is JSON Lines, appended as each file is created, so a run that is
//! interrupted (app quit, crash, power loss) can still be undone up to its last file.

use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::SystemTime;

use serde::{Deserialize, Serialize};

use crate::config::app_data_dir;
use crate::error::{io_err, CoreError, Result};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManifestEntry {
    pub path: PathBuf,
    pub size: u64,
    pub modified: SystemTime,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Manifest {
    pub files: Vec<ManifestEntry>,
    /// Folders the conversion created (they did not exist before).
    pub dirs: Vec<PathBuf>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UndoReport {
    pub removed: usize,
    pub kept_modified: Vec<PathBuf>,
    pub failed: Vec<PathBuf>,
    pub missing: usize,
    pub removed_dirs: usize,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
enum ManifestLine {
    Dir { path: PathBuf },
    File(ManifestEntry),
}

impl Manifest {
    pub fn save_to(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(io_err(parent))?;
        }
        let mut text = String::new();
        let lines = self
            .dirs
            .iter()
            .map(|d| ManifestLine::Dir { path: d.clone() })
            .chain(self.files.iter().cloned().map(ManifestLine::File));
        for line in lines {
            text.push_str(&serde_json::to_string(&line).map_err(|e| CoreError::Internal(e.to_string()))?);
            text.push('\n');
        }
        fs::write(path, text).map_err(io_err(path))
    }

    /// Unreadable lines (e.g. a half-written last line after a crash) are skipped.
    pub fn load_from(path: &Path) -> Result<Option<Manifest>> {
        if !path.exists() {
            return Ok(None);
        }
        let text = fs::read_to_string(path).map_err(io_err(path))?;
        let mut manifest = Manifest::default();
        for line in text.lines().filter(|l| !l.trim().is_empty()) {
            match serde_json::from_str::<ManifestLine>(line) {
                Ok(ManifestLine::Dir { path }) => manifest.dirs.push(path),
                Ok(ManifestLine::File(entry)) => manifest.files.push(entry),
                Err(_) => continue,
            }
        }
        Ok(Some(manifest))
    }
}

/// Appends manifest lines while a run is in progress. The file is created (replacing the
/// previous session) only on the first record, so a run that creates nothing keeps the
/// previous session undoable.
#[derive(Debug)]
pub struct ManifestWriter {
    path: PathBuf,
    file: Mutex<Option<File>>,
}

impl ManifestWriter {
    pub fn new(path: &Path) -> Self {
        ManifestWriter { path: path.to_path_buf(), file: Mutex::new(None) }
    }

    pub fn record_dir(&self, dir: &Path) -> Result<()> {
        self.append(&ManifestLine::Dir { path: dir.to_path_buf() })
    }

    pub fn record_file(&self, path: &Path) -> Result<()> {
        match record(path) {
            Some(entry) => self.append(&ManifestLine::File(entry)),
            None => Ok(()),
        }
    }

    fn append(&self, line: &ManifestLine) -> Result<()> {
        let json = serde_json::to_string(line).map_err(|e| CoreError::Internal(e.to_string()))?;
        let mut guard = self.file.lock().map_err(|_| CoreError::Internal("manifest lock poisoned".into()))?;
        if guard.is_none() {
            if let Some(parent) = self.path.parent() {
                fs::create_dir_all(parent).map_err(io_err(parent))?;
            }
            *guard = Some(File::create(&self.path).map_err(io_err(&self.path))?);
        }
        let file = guard.as_mut().expect("opened above");
        writeln!(file, "{json}").map_err(io_err(&self.path))
    }
}

pub fn record(path: &Path) -> Option<ManifestEntry> {
    let meta = fs::metadata(path).ok()?;
    Some(ManifestEntry { path: path.to_path_buf(), size: meta.len(), modified: meta.modified().ok()? })
}

pub fn undo(manifest: &Manifest, remove_empty_dirs: bool) -> UndoReport {
    let mut report = UndoReport::default();
    for entry in &manifest.files {
        match fs::metadata(&entry.path) {
            Err(_) => report.missing += 1,
            Ok(meta) => {
                let unchanged = meta.len() == entry.size && meta.modified().ok() == Some(entry.modified);
                if !unchanged {
                    report.kept_modified.push(entry.path.clone());
                } else if fs::remove_file(&entry.path).is_ok() {
                    report.removed += 1;
                } else {
                    report.failed.push(entry.path.clone());
                }
            }
        }
    }
    if remove_empty_dirs {
        let mut dirs = manifest.dirs.clone();
        dirs.sort_by_key(|d| std::cmp::Reverse(d.components().count()));
        for dir in dirs {
            // remove_dir only succeeds on empty folders, so user files are never touched.
            if fs::remove_dir(&dir).is_ok() {
                report.removed_dirs += 1;
            }
        }
    }
    report
}

pub fn manifest_path() -> PathBuf {
    app_data_dir().join("last_session.json")
}

pub fn undo_last_session(manifest_file: &Path, remove_empty_dirs: bool) -> Result<Option<UndoReport>> {
    let Some(manifest) = Manifest::load_from(manifest_file)? else {
        return Ok(None);
    };
    let report = undo(&manifest, remove_empty_dirs);
    fs::remove_file(manifest_file).map_err(io_err(manifest_file))?;
    Ok(Some(report))
}

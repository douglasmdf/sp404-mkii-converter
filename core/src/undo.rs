//! Records what a conversion created so it can be undone safely: a file is deleted only
//! if its size and modification time still match what was recorded.

use std::fs;
use std::path::{Path, PathBuf};
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

impl Manifest {
    pub fn save_to(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(io_err(parent))?;
        }
        let json = serde_json::to_string_pretty(self).map_err(|e| CoreError::Internal(e.to_string()))?;
        fs::write(path, json).map_err(io_err(path))
    }

    pub fn load_from(path: &Path) -> Result<Option<Manifest>> {
        if !path.exists() {
            return Ok(None);
        }
        let text = fs::read_to_string(path).map_err(io_err(path))?;
        serde_json::from_str(&text)
            .map(Some)
            .map_err(|e| CoreError::Internal(format!("corrupt undo manifest: {e}")))
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

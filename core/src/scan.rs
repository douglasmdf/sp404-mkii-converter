//! Walks pack folders and builds a `Plan`: every source file, where it goes and how.
//! Nothing here writes to disk.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::config::{SaveMode, Settings};
use crate::error::{io_err, CoreError, Result};
use crate::naming::{flatten_stem, sanitize_component, CollisionResolver};

pub const CONVERTIBLE_EXTENSIONS: &[&str] = &[
    "wav", "wave", "aif", "aiff", "aifc", "flac", "mp3", "m4a", "aac", "ogg", "oga", "caf",
];
pub const UNSUPPORTED_AUDIO_EXTENSIONS: &[&str] =
    &["opus", "wma", "ape", "wv", "mpc", "ra", "amr", "dsf", "dff"];
const IGNORED_NAMES: &[&str] = &["__MACOSX", "Thumbs.db", "desktop.ini"];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum JobKind {
    Convert,
    Copy,
    Unsupported,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Job {
    pub source: PathBuf,
    pub dest: PathBuf,
    pub kind: JobKind,
    pub already_exists: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PackPlan {
    pub name: String,
    pub source_root: PathBuf,
    pub dest_root: PathBuf,
    pub jobs: Vec<Job>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Plan {
    pub packs: Vec<PackPlan>,
}

impl Plan {
    pub fn jobs(&self) -> impl Iterator<Item = &Job> + '_ {
        self.packs.iter().flat_map(|p| p.jobs.iter())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PackSummary {
    pub name: String,
    pub path: PathBuf,
    pub audio_files: usize,
    pub other_files: usize,
    pub unsupported_files: usize,
}

pub fn is_ignored(name: &str) -> bool {
    name.starts_with('.') || IGNORED_NAMES.iter().any(|n| n.eq_ignore_ascii_case(name))
}

pub fn classify(path: &Path) -> JobKind {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .unwrap_or_default();
    if CONVERTIBLE_EXTENSIONS.contains(&ext.as_str()) {
        JobKind::Convert
    } else if UNSUPPORTED_AUDIO_EXTENSIONS.contains(&ext.as_str()) {
        JobKind::Unsupported
    } else {
        JobKind::Copy
    }
}

pub fn inspect_pack(dir: &Path) -> Result<PackSummary> {
    if !dir.is_dir() {
        return Err(CoreError::NotADirectory(dir.to_path_buf()));
    }
    let mut summary = PackSummary {
        name: dir_name(dir),
        path: dir.to_path_buf(),
        audio_files: 0,
        other_files: 0,
        unsupported_files: 0,
    };
    for rel in collect_files(dir)? {
        match classify(&rel) {
            JobKind::Convert => summary.audio_files += 1,
            JobKind::Copy => summary.other_files += 1,
            JobKind::Unsupported => summary.unsupported_files += 1,
        }
    }
    Ok(summary)
}

pub fn build_plan(pack_dirs: &[PathBuf], output_dir: Option<&Path>, settings: &Settings) -> Result<Plan> {
    let mut used_roots = HashSet::new();
    let mut packs = Vec::with_capacity(pack_dirs.len());
    for source_root in pack_dirs {
        if !source_root.is_dir() {
            return Err(CoreError::NotADirectory(source_root.clone()));
        }
        let base_name = pack_name(source_root, settings);
        let mut n = 1usize;
        let (name, dest_root) = loop {
            let name = if n == 1 { base_name.clone() } else { format!("{base_name}_{n}") };
            let dest_root = pack_dest_root(source_root, &name, output_dir, settings)?;
            if used_roots.insert(dest_root.to_string_lossy().to_lowercase()) {
                break (name, dest_root);
            }
            n += 1;
        };
        ensure_outside(&dest_root, source_root)?;
        packs.push(plan_pack(source_root, name, dest_root, settings)?);
    }
    Ok(Plan { packs })
}

fn plan_pack(source_root: &Path, name: String, dest_root: PathBuf, settings: &Settings) -> Result<PackPlan> {
    let mut resolver = CollisionResolver::new();
    let mut jobs = Vec::new();
    for rel in collect_files(source_root)? {
        let kind = classify(&rel);
        let dirs: Vec<String> = rel
            .parent()
            .map(|p| p.iter().map(|c| c.to_string_lossy().into_owned()).collect())
            .unwrap_or_default();
        let stem = rel.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
        let original_ext = rel.extension().map(|e| e.to_string_lossy().into_owned()).unwrap_or_default();

        let rel_dir: PathBuf = if settings.flatten_folders {
            PathBuf::new()
        } else {
            dirs.iter()
                .map(|d| if settings.sanitize_folder_names { sanitize_component(d) } else { d.clone() })
                .collect()
        };
        let stem = if settings.flatten_folders { flatten_stem(&dirs, &stem) } else { stem };
        let (stem, ext, max_len) = match kind {
            JobKind::Convert => {
                let stem = if settings.sanitize_filenames { sanitize_component(&stem) } else { stem };
                (stem, String::from("wav"), settings.max_filename_length)
            }
            JobKind::Copy | JobKind::Unsupported => (stem, original_ext, None),
        };
        let file_name = resolver.claim(&rel_dir, &stem, &ext, max_len);
        let dest = dest_root.join(&rel_dir).join(&file_name);
        let already_exists = dest.exists();
        jobs.push(Job { source: source_root.join(&rel), dest, kind, already_exists });
    }
    Ok(PackPlan { name, source_root: source_root.to_path_buf(), dest_root, jobs })
}

fn dir_name(dir: &Path) -> String {
    dir.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| String::from("pack"))
}

fn pack_name(source_root: &Path, settings: &Settings) -> String {
    let raw = dir_name(source_root);
    if settings.sanitize_folder_names { sanitize_component(&raw) } else { raw }
}

fn pack_dest_root(source_root: &Path, name: &str, output_dir: Option<&Path>, settings: &Settings) -> Result<PathBuf> {
    match settings.save_mode {
        SaveMode::Ask => Ok(output_dir.ok_or(CoreError::NoOutputDir)?.join(name)),
        SaveMode::Fixed => Ok(settings
            .fixed_directory
            .as_deref()
            .ok_or(CoreError::NoOutputDir)?
            .join(name)),
        SaveMode::NextToSource => {
            let parent = source_root.parent().ok_or(CoreError::NoOutputDir)?;
            Ok(parent.join(format!("{name} [SP-404]")))
        }
    }
}

fn ensure_outside(dest_root: &Path, source_root: &Path) -> Result<()> {
    if resolve_path(dest_root).starts_with(resolve_path(source_root)) {
        return Err(CoreError::OutputInsideSource {
            dest: dest_root.to_path_buf(),
            source_dir: source_root.to_path_buf(),
        });
    }
    Ok(())
}

/// Absolute path with symlinks resolved for the part that exists (so `/var` and
/// `/private/var` on macOS compare equal), keeping the not-yet-created tail as is.
fn resolve_path(path: &Path) -> PathBuf {
    let absolute = std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf());
    let mut existing = absolute.as_path();
    let mut tail = Vec::new();
    while !existing.exists() {
        match (existing.parent(), existing.file_name()) {
            (Some(parent), Some(name)) => {
                tail.push(name.to_os_string());
                existing = parent;
            }
            _ => break,
        }
    }
    let mut resolved = existing.canonicalize().unwrap_or_else(|_| existing.to_path_buf());
    for name in tail.iter().rev() {
        resolved.push(name);
    }
    resolved
}

/// Relative paths of every regular file under `root`, sorted, skipping system junk.
/// Symlinks are not followed.
fn collect_files(root: &Path) -> Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    walk(root, Path::new(""), &mut out)?;
    out.sort();
    Ok(out)
}

fn walk(root: &Path, rel: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
    let dir = root.join(rel);
    for entry in fs::read_dir(&dir).map_err(io_err(&dir))? {
        let entry = entry.map_err(io_err(&dir))?;
        let name = entry.file_name();
        if is_ignored(&name.to_string_lossy()) {
            continue;
        }
        let file_type = entry.file_type().map_err(io_err(&dir))?;
        let child = rel.join(&name);
        if file_type.is_dir() {
            walk(root, &child, out)?;
        } else if file_type.is_file() {
            out.push(child);
        }
    }
    Ok(())
}

//! Walks pack folders and builds a `Plan`: every source file, where it goes and how.
//! Nothing here writes to disk.
//!
//! Building a plan has three steps: collect the files of every pack and of the loose files;
//! with key/BPM detection on, analyse the convertible ones in parallel; then decide every
//! name in order (collisions are resolved sequentially, so names are deterministic).

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use rayon::prelude::*;
use serde::{Deserialize, Serialize};

use crate::analyze::{AnalysisCache, NameHints};
use crate::config::{SaveMode, Settings};
use crate::error::{io_err, CoreError, Result};
use crate::labeling::{self, Analysis};
use crate::name_tags::{parse_name, remove_spans, Span};
use crate::naming::{flatten_stem, sanitize_component, CollisionResolver};
use crate::tags::prepend;

pub const CONVERTIBLE_EXTENSIONS: &[&str] = &[
    "wav", "wave", "aif", "aiff", "aifc", "flac", "mp3", "m4a", "aac", "ogg", "oga", "caf",
];
pub const UNSUPPORTED_AUDIO_EXTENSIONS: &[&str] =
    &["opus", "wma", "ape", "wv", "mpc", "ra", "amr", "dsf", "dff"];
/// Folder (inside the chosen output folder) that receives files dropped one by one.
pub const LOOSE_FOLDER_NAME: &str = "Samples [SP-404]";
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
    /// Key/BPM detection result; only with detection on, only for convertible files.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub analysis: Option<Analysis>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PackPlan {
    pub name: String,
    pub source_root: PathBuf,
    pub dest_root: PathBuf,
    pub jobs: Vec<Job>,
    /// True for the group of files dropped individually (not inside a pack folder).
    pub loose: bool,
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InputsSummary {
    pub packs: Vec<PackSummary>,
    pub loose: Option<PackSummary>,
    pub rejected: Vec<PathBuf>,
}

/// Progress while a plan is built (only reported when audio is analysed).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "event", rename_all = "camelCase")]
pub enum PlanProgress {
    Analyzing { done: usize, total: usize },
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

/// Sorts dropped paths into pack folders and loose files; a path that is neither is `NotFound`.
fn split_inputs(paths: &[PathBuf]) -> Result<(Vec<&PathBuf>, Vec<&PathBuf>)> {
    let (mut dirs, mut files) = (Vec::new(), Vec::new());
    for path in paths {
        if path.is_dir() {
            dirs.push(path);
        } else if path.is_file() {
            files.push(path);
        } else {
            return Err(CoreError::NotFound(path.clone()));
        }
    }
    Ok((dirs, files))
}

pub fn inspect_inputs(paths: &[PathBuf]) -> Result<InputsSummary> {
    let (dirs, files) = split_inputs(paths)?;
    let packs = dirs.into_iter().map(|d| inspect_pack(d)).collect::<Result<Vec<_>>>()?;
    let mut loose = PackSummary {
        name: LOOSE_FOLDER_NAME.to_string(),
        path: PathBuf::new(),
        audio_files: 0,
        other_files: 0,
        unsupported_files: 0,
    };
    let mut rejected = Vec::new();
    for file in files {
        match classify(file) {
            JobKind::Convert => loose.audio_files += 1,
            JobKind::Unsupported => loose.unsupported_files += 1,
            JobKind::Copy => rejected.push(file.clone()),
        }
    }
    let has_loose = loose.audio_files + loose.unsupported_files > 0;
    Ok(InputsSummary { packs, loose: has_loose.then_some(loose), rejected })
}

pub fn build_plan(inputs: &[PathBuf], output_dir: Option<&Path>, settings: &Settings) -> Result<Plan> {
    plan_with(inputs, output_dir, settings, &AnalysisCache::default(), &AtomicBool::new(false), |_| {})
}

/// `build_plan` with an analysis cache, a cancel flag (checked before each analysed file)
/// and progress reports while files are analysed.
pub fn plan_with(
    inputs: &[PathBuf],
    output_dir: Option<&Path>,
    settings: &Settings,
    cache: &AnalysisCache,
    cancel: &AtomicBool,
    on_progress: impl Fn(PlanProgress) + Sync,
) -> Result<Plan> {
    let (pack_dirs, files) = split_inputs(inputs)?;
    for file in &files {
        if classify(file) == JobKind::Copy {
            return Err(CoreError::NotAudioFile((*file).clone()));
        }
    }
    let loose_root = if files.is_empty() {
        None
    } else {
        Some(output_dir.ok_or(CoreError::NoOutputDir)?.join(LOOSE_FOLDER_NAME))
    };

    let mut used_roots = HashSet::new();
    // Reserved first so a pack folder named like the loose folder gets a numbered root instead.
    if let Some(root) = &loose_root {
        used_roots.insert(root.to_string_lossy().to_lowercase());
    }
    let mut roots = Vec::with_capacity(pack_dirs.len());
    for source_root in pack_dirs {
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
        roots.push((source_root, name, dest_root));
    }
    let mut sources: Vec<&PathBuf> = roots.iter().map(|(src, _, _)| *src).collect();
    sources.extend(files.iter().copied());
    let mut dests: Vec<&PathBuf> = roots.iter().map(|(_, _, dest)| dest).collect();
    dests.extend(loose_root.as_ref());
    ensure_no_overlap(&sources, &dests)?;

    let mut groups = Vec::with_capacity(roots.len() + 1);
    for (source_root, name, dest_root) in roots {
        groups.push(collect_pack(source_root, name, dest_root)?);
    }
    if let Some(dest_root) = loose_root {
        groups.push(collect_loose(&files, dest_root));
    }
    let analyses = if settings.detect_key_bpm {
        analyze_groups(&groups, settings, cache, cancel, &on_progress)?
    } else {
        groups.iter().map(|g| vec![None; g.entries.len()]).collect()
    };
    let packs = groups.into_iter().zip(analyses).map(|(group, found)| name_group(group, found, settings)).collect();
    Ok(Plan { packs })
}

/// A file of a pack (or a loose file) before its output name is decided.
struct Entry {
    source: PathBuf,
    kind: JobKind,
    /// Source subfolders down to the file (raw names); empty for loose files.
    dirs: Vec<String>,
    stem: String,
    ext: String,
    /// Folders whose names count as classification hints.
    hint_dirs: Vec<String>,
}

struct Group {
    name: String,
    source_root: PathBuf,
    dest_root: PathBuf,
    loose: bool,
    entries: Vec<Entry>,
}

type Found = Option<(Analysis, Vec<Span>)>;

fn collect_pack(source_root: &Path, name: String, dest_root: PathBuf) -> Result<Group> {
    // Hints use the pack folder and its subfolders, never the folders above the pack.
    let pack_folder = dir_name(source_root);
    let entries = collect_files(source_root)?
        .into_iter()
        .map(|rel| {
            let dirs: Vec<String> = rel
                .parent()
                .map(|p| p.iter().map(|c| c.to_string_lossy().into_owned()).collect())
                .unwrap_or_default();
            let mut hint_dirs = vec![pack_folder.clone()];
            hint_dirs.extend(dirs.iter().cloned());
            Entry {
                source: source_root.join(&rel),
                kind: classify(&rel),
                dirs,
                stem: file_stem(&rel),
                ext: file_ext(&rel),
                hint_dirs,
            }
        })
        .collect();
    Ok(Group { name, source_root: source_root.to_path_buf(), dest_root, loose: false, entries })
}

/// All loose files share one flat folder; names follow the same rules as inside a pack.
fn collect_loose(files: &[&PathBuf], dest_root: PathBuf) -> Group {
    let entries = files
        .iter()
        .map(|file| {
            let source = std::path::absolute(file).unwrap_or_else(|_| (*file).clone());
            // Only the folder the file sits in counts as a hint.
            let hint_dirs = source
                .parent()
                .and_then(|p| p.file_name())
                .map(|n| vec![n.to_string_lossy().into_owned()])
                .unwrap_or_default();
            Entry { kind: classify(file), dirs: Vec::new(), stem: file_stem(file), ext: file_ext(file), hint_dirs, source }
        })
        .collect();
    Group { name: LOOSE_FOLDER_NAME.to_string(), source_root: PathBuf::new(), dest_root, loose: true, entries }
}

fn file_stem(path: &Path) -> String {
    path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default()
}

fn file_ext(path: &Path) -> String {
    path.extension().map(|e| e.to_string_lossy().into_owned()).unwrap_or_default()
}

/// Analyses every convertible file on a pool of `settings.workers` threads. A file that
/// cannot be analysed gets `labeling::failed`; a set `cancel` stops with `Cancelled`.
fn analyze_groups(
    groups: &[Group],
    settings: &Settings,
    cache: &AnalysisCache,
    cancel: &AtomicBool,
    on_progress: &(impl Fn(PlanProgress) + Sync),
) -> Result<Vec<Vec<Found>>> {
    if cancel.load(Ordering::SeqCst) {
        return Err(CoreError::Cancelled);
    }
    let tasks: Vec<(usize, usize)> = groups
        .iter()
        .enumerate()
        .flat_map(|(g, group)| group.entries.iter().enumerate().filter(|(_, e)| e.kind == JobKind::Convert).map(move |(i, _)| (g, i)))
        .collect();
    let total = tasks.len();
    let done = AtomicUsize::new(0);
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(settings.workers.max(1))
        .build()
        .map_err(|e| CoreError::Internal(e.to_string()))?;
    let results: Vec<((usize, usize), (Analysis, Vec<Span>))> = pool.install(|| {
        tasks
            .par_iter()
            .map(|&(g, i)| {
                if cancel.load(Ordering::SeqCst) {
                    return Err(CoreError::Cancelled);
                }
                let entry = &groups[g].entries[i];
                let tokens = parse_name(&entry.stem);
                let hints = NameHints::from_path(&entry.hint_dirs, &entry.stem, &tokens);
                let found = match cache.get_or_analyze(&entry.source, &hints) {
                    Ok(detection) => labeling::resolve(&detection, &tokens, settings.bpm_range),
                    Err(e) => (labeling::failed(&e.to_string()), Vec::new()),
                };
                let done_now = done.fetch_add(1, Ordering::SeqCst) + 1;
                on_progress(PlanProgress::Analyzing { done: done_now, total });
                Ok(((g, i), found))
            })
            .collect::<Result<Vec<_>>>()
    })?;
    let mut out: Vec<Vec<Found>> = groups.iter().map(|g| vec![None; g.entries.len()]).collect();
    for ((g, i), found) in results {
        out[g][i] = Some(found);
    }
    Ok(out)
}

/// Decides every output name of a group, in order.
fn name_group(group: Group, found: Vec<Found>, settings: &Settings) -> PackPlan {
    let mut resolver = CollisionResolver::new();
    let jobs = group
        .entries
        .into_iter()
        .zip(found)
        .map(|(entry, found)| name_entry(&mut resolver, entry, found, &group.dest_root, settings))
        .collect();
    PackPlan { name: group.name, source_root: group.source_root, dest_root: group.dest_root, jobs, loose: group.loose }
}

/// Name order: remove the name tokens the analysis used, flatten folders into the name,
/// clean it, put the prefix in front, then apply the length limit and collisions.
fn name_entry(resolver: &mut CollisionResolver, entry: Entry, found: Found, dest_root: &Path, settings: &Settings) -> Job {
    let (analysis, spans) = match found {
        Some((analysis, spans)) => (Some(analysis), spans),
        None => (None, Vec::new()),
    };
    let prefix = analysis.as_ref().and_then(|a| a.prefix.clone());
    let stem = remove_spans(&entry.stem, &spans);
    let rel_dir: PathBuf = if settings.flatten_folders {
        PathBuf::new()
    } else {
        entry
            .dirs
            .iter()
            .map(|d| if settings.sanitize_folder_names { sanitize_component(d) } else { d.clone() })
            .collect()
    };
    let stem = match (settings.flatten_folders, stem.is_empty()) {
        (true, false) => flatten_stem(&entry.dirs, &stem),
        (true, true) => entry.dirs.join("_"),
        (false, _) => stem,
    };
    let (stem, ext, max_len) = match entry.kind {
        JobKind::Convert => {
            // A name that was only key/BPM becomes just the prefix (`Am 90bpm` → `Am_90`).
            let only_prefix = stem.is_empty() && prefix.is_some();
            let stem = if settings.sanitize_filenames && !only_prefix { sanitize_component(&stem) } else { stem };
            (prepend(prefix.as_deref(), &stem), String::from("wav"), settings.max_filename_length)
        }
        JobKind::Copy | JobKind::Unsupported => (stem, entry.ext, None),
    };
    let file_name = resolver.claim(&rel_dir, &stem, &ext, max_len);
    let dest = dest_root.join(&rel_dir).join(&file_name);
    let already_exists = dest.exists();
    Job { source: entry.source, dest, kind: entry.kind, already_exists, analysis }
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

/// No output folder may equal, sit inside, or contain any source (folder or loose file) of the
/// batch — otherwise converted files would mix with (or land among) someone's originals.
fn ensure_no_overlap(sources: &[&PathBuf], dests: &[&PathBuf]) -> Result<()> {
    let sources: Vec<(&PathBuf, PathBuf)> = sources.iter().map(|src| (*src, resolve_path(src))).collect();
    for dest_root in dests {
        let dest = resolve_path(dest_root);
        for (source_root, source) in &sources {
            if dest.starts_with(source) || source.starts_with(&dest) {
                return Err(CoreError::OutputInsideSource {
                    dest: (*dest_root).clone(),
                    source_dir: (*source_root).clone(),
                });
            }
        }
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

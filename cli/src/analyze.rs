//! `sp404 analyze`: runs the detector on audio files, prints one line per file and
//! optionally scores the results against a CSV answer key.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

use rayon::prelude::*;
use sp404_core::analyze::{analyze_file, NameHints};
use sp404_core::config::BpmRange;
use sp404_core::labeling::{self, Analysis};
use sp404_core::name_tags::{parse_name, NameTokens};
use sp404_core::scan::{self, JobKind};

use crate::eval::{self, Label};

pub struct Options {
    pub inputs: Vec<PathBuf>,
    pub range: BpmRange,
    pub ignore_names: bool,
    pub labels: Option<PathBuf>,
    pub report: Option<PathBuf>,
}

struct Item {
    /// Path as printed (the input folder joined with the relative path).
    shown: String,
    /// Path relative to the input folder (just the file name for a loose file).
    rel: String,
    source: PathBuf,
    hint_dirs: Vec<String>,
    stem: String,
}

fn name_of(path: &Path) -> Option<String> {
    path.file_name().map(|n| n.to_string_lossy().into_owned())
}

fn walk(root: &Path, rel: &Path, out: &mut Vec<PathBuf>) -> Result<(), String> {
    let dir = root.join(rel);
    for entry in fs::read_dir(&dir).map_err(|e| format!("{}: {e}", dir.display()))? {
        let entry = entry.map_err(|e| format!("{}: {e}", dir.display()))?;
        let name = entry.file_name();
        if scan::is_ignored(&name.to_string_lossy()) {
            continue;
        }
        let kind = entry.file_type().map_err(|e| e.to_string())?;
        let child = rel.join(&name);
        if kind.is_dir() {
            walk(root, &child, out)?;
        } else if kind.is_file() && scan::classify(&child) == JobKind::Convert {
            out.push(child);
        }
    }
    Ok(())
}

fn collect(inputs: &[PathBuf]) -> Result<Vec<Item>, String> {
    let mut items = Vec::new();
    for input in inputs {
        if input.is_dir() {
            let root_name = name_of(&std::path::absolute(input).unwrap_or_else(|_| input.clone())).unwrap_or_default();
            let mut files = Vec::new();
            walk(input, Path::new(""), &mut files)?;
            for rel in files {
                let mut hint_dirs = vec![root_name.clone()];
                if let Some(parent) = rel.parent() {
                    hint_dirs.extend(parent.components().map(|c| c.as_os_str().to_string_lossy().into_owned()));
                }
                items.push(Item {
                    shown: input.join(&rel).display().to_string(),
                    rel: rel.components().map(|c| c.as_os_str().to_string_lossy()).collect::<Vec<_>>().join("/"),
                    stem: rel.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default(),
                    source: input.join(&rel),
                    hint_dirs,
                });
            }
        } else if input.is_file() {
            if scan::classify(input) != JobKind::Convert {
                return Err(format!("not an audio file: {}", input.display()));
            }
            let absolute = std::path::absolute(input).unwrap_or_else(|_| input.clone());
            let hint_dirs = absolute.parent().and_then(name_of).into_iter().collect();
            items.push(Item {
                shown: input.display().to_string(),
                rel: name_of(input).unwrap_or_default(),
                stem: input.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default(),
                source: input.clone(),
                hint_dirs,
            });
        } else {
            return Err(format!("not found: {}", input.display()));
        }
    }
    items.sort_by(|a, b| a.shown.cmp(&b.shown));
    Ok(items)
}

fn run_one(item: &Item, opts: &Options) -> (Analysis, f64) {
    let started = Instant::now();
    let (tokens, hints) = if opts.ignore_names {
        (NameTokens::default(), NameHints::default())
    } else {
        let tokens = parse_name(&item.stem);
        let hints = NameHints::from_path(&item.hint_dirs, &item.stem, &tokens);
        (tokens, hints)
    };
    let analysis = match analyze_file(&item.source, &hints) {
        Ok(detection) => labeling::resolve(&detection, &tokens, opts.range).0,
        Err(e) => labeling::failed(&e.to_string()),
    };
    (analysis, started.elapsed().as_secs_f64() * 1000.0)
}

fn line(item: &Item, a: &Analysis) -> String {
    if let Some(error) = &a.error {
        return format!("{}  (not analyzed: {error})", item.shown);
    }
    let mut out = format!(
        "{}  {}  {}  {}",
        item.shown,
        eval::kind_name(a.kind),
        a.label.as_deref().unwrap_or("-"),
        a.bpm.map_or(String::from("-"), |b| b.to_string())
    );
    for (flag, text) in [(a.kind_uncertain, "type"), (a.label_uncertain, "key"), (a.bpm_uncertain, "bpm")] {
        if flag {
            out.push_str(&format!("  ({text} uncertain)"));
        }
    }
    out.push_str(if a.label_from_name || a.bpm_from_name { "  (name)" } else { "  (audio)" });
    out
}

pub fn run(opts: &Options) -> Result<(), String> {
    let answer_key = match &opts.labels {
        Some(path) => Some(eval::parse_labels(&fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?)?),
        None => None,
    };
    let items = collect(&opts.inputs)?;
    let results: Vec<(Analysis, f64)> = items.par_iter().map(|item| run_one(item, opts)).collect();
    for (item, (analysis, _)) in items.iter().zip(&results) {
        println!("{}", line(item, analysis));
    }
    let Some(answer_key) = answer_key else {
        return Ok(());
    };
    let mut by_rel: BTreeMap<&str, Vec<usize>> = BTreeMap::new();
    for (i, item) in items.iter().enumerate() {
        by_rel.entry(item.rel.as_str()).or_default().push(i);
    }
    let mut scored: Vec<(Label, Analysis)> = Vec::new();
    let mut millis = Vec::new();
    for label in answer_key {
        let key = label.path.replace('\\', "/");
        match by_rel.get(key.trim_start_matches("./")) {
            Some(indexes) => {
                for &i in indexes {
                    scored.push((label.clone(), results[i].0.clone()));
                    millis.push(results[i].1);
                }
            }
            None => eprintln!("warning: no analysed file matches the answer key path {}", label.path),
        }
    }
    let mut metrics = eval::score(&scored, opts.range);
    if !millis.is_empty() {
        metrics.mean_millis = Some(millis.iter().sum::<f64>() / millis.len() as f64);
    }
    let report = metrics.report();
    println!("\n{report}");
    if let Some(path) = &opts.report {
        fs::write(path, &report).map_err(|e| format!("{}: {e}", path.display()))?;
    }
    Ok(())
}

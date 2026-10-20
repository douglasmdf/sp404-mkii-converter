//! Audio analysis for the key/BPM prefix: note, chord, key and tempo of a sample.
//! Pure Rust, deterministic (no randomness, no dependence on thread count).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;
use std::time::SystemTime;

use serde::Serialize;

use crate::error::{CoreError, Result};
use crate::name_tags::NameTokens;
use crate::tags::{Chord, Key, PitchClass, SampleKind};

pub mod classify;
pub mod features;
pub mod harmony;
pub mod pitch;
pub mod signal;
pub mod tempo;
#[cfg(test)]
pub(crate) mod synth;

/// Every file is analysed at this rate, whatever its source rate.
pub const ANALYSIS_RATE: u32 = 22_050;
/// Only the start of a file is decoded and analysed.
pub const MAX_ANALYSIS_SECONDS: f64 = 180.0;
/// Bumped whenever thresholds or profiles change, so logs show which analyzer named a file.
pub const ANALYZER_VERSION: u32 = 7;

/// A detected value and whether the analyzer trusts it enough to name the file with it.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Estimate<T> {
    pub value: T,
    pub confident: bool,
}

/// Words from the folder and file names, plus whether the name already carries a key or BPM.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct NameHints {
    pub words: Vec<String>,
    pub has_key_token: bool,
    pub has_bpm_token: bool,
    /// The marked BPM of the name, in hundredths (kept whole so hints stay hashable).
    pub name_bpm_centi: Option<u32>,
}

impl NameHints {
    /// `rel_dirs` are the folders that count (the caller picks them: for a pack, the pack
    /// folder and its subfolders down to the file; for a loose file, only its own folder).
    pub fn from_path(rel_dirs: &[String], stem: &str, tokens: &NameTokens) -> NameHints {
        let mut words = Vec::new();
        for text in rel_dirs.iter().map(String::as_str).chain([stem]) {
            words.extend(hint_words(text));
        }
        NameHints {
            words,
            has_key_token: tokens.key.is_some() || tokens.chord.is_some(),
            has_bpm_token: tokens.bpm.is_some(),
            name_bpm_centi: tokens.bpm.as_ref().map(|(b, _)| (b * 100.0).round() as u32),
        }
    }

    pub fn name_bpm(&self) -> Option<f32> {
        self.name_bpm_centi.map(|c| c as f32 / 100.0)
    }

    /// Whether `word` (a hint list entry such as `one shot` or `hi-hat`) is among the words.
    pub fn has_word(&self, word: &str) -> bool {
        let wanted = hint_words(word);
        !wanted.is_empty() && self.words.windows(wanted.len()).any(|w| w == wanted.as_slice())
    }
}

/// Lowercase words, split at anything that is not a letter or digit and between letters and
/// digits (`Kick01` → `kick`, `01`), with "one shot(s)" folded into `oneshot`.
fn hint_words(text: &str) -> Vec<String> {
    let lower = text.to_lowercase();
    let mut words = Vec::new();
    let mut current = String::new();
    for c in lower.chars() {
        let boundary = current.chars().last().is_some_and(|p| p.is_ascii_digit() != c.is_ascii_digit());
        if !c.is_alphanumeric() || boundary {
            if !current.is_empty() {
                words.push(std::mem::take(&mut current));
            }
        }
        if c.is_alphanumeric() {
            current.push(c);
        }
    }
    if !current.is_empty() {
        words.push(current);
    }
    fold_one_shot(words)
}

/// `one` + `shot`/`shots` (separate words) and `oneshots` become `oneshot`.
fn fold_one_shot(words: Vec<String>) -> Vec<String> {
    let mut out: Vec<String> = Vec::with_capacity(words.len());
    for word in words {
        let joins = matches!(word.as_str(), "shot" | "shots") && out.last().is_some_and(|w| w == "one");
        if joins {
            out.pop();
        }
        if joins || word == "oneshots" {
            out.push(String::from("oneshot"));
        } else {
            out.push(word);
        }
    }
    out
}

/// What the analyzer found. Only the fields the kind uses are filled.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Detection {
    pub kind: Estimate<SampleKind>,
    pub note: Option<Estimate<PitchClass>>,
    pub chord: Option<Estimate<Chord>>,
    pub key: Option<Estimate<Key>>,
    /// Raw BPM, not yet folded into a range.
    pub bpm: Option<Estimate<f32>>,
}

/// Decodes and analyses one file. A file with no audible sound (empty or silent) is an error.
pub fn analyze_file(path: &Path, hints: &NameHints) -> Result<Detection> {
    let signal = signal::load(path)?;
    if signal.samples.is_empty() {
        return Err(CoreError::NoAudio);
    }
    Ok(analyze_signal(&signal, hints))
}

pub(crate) fn analyze_signal(signal: &signal::Signal, hints: &NameHints) -> Detection {
    let features = features::extract(signal);
    let rhythm = tempo::Rhythm::new(&features, signal.loop_seconds());
    let kind = classify::classify_with(signal, &features, hints, rhythm.periodicity());
    let mut detection = Detection { kind: kind.clone(), note: None, chord: None, key: None, bpm: None };
    match kind.value {
        SampleKind::Unpitched => {}
        SampleKind::Note => detection.note = pitch::one_shot_note(signal, &features),
        SampleKind::Chord => detection.chord = harmony::estimate_chord(&features),
        SampleKind::Phrase => detection.key = harmony::estimate_key(&features),
        SampleKind::MelodicLoop => {
            detection.key = harmony::estimate_key(&features);
            detection.bpm = rhythm.bpm();
        }
        SampleKind::DrumLoop => detection.bpm = rhythm.bpm(),
    }
    detection
}

/// Detections kept in memory between plans (a Dry Run then a Convert analyse each file once).
/// Keyed by path, size, modification time and hints (hints change the classification); the
/// BPM range is applied afterwards, so it is not part of the key. Failures are kept too (as
/// their message), so a broken file is not decoded again.
#[derive(Debug, Default)]
pub struct AnalysisCache {
    entries: Mutex<HashMap<CacheKey, std::result::Result<Detection, String>>>,
    hits: AtomicUsize,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct CacheKey {
    path: PathBuf,
    size: u64,
    modified: Option<SystemTime>,
    hints: NameHints,
}

impl AnalysisCache {
    pub fn get_or_analyze(&self, path: &Path, hints: &NameHints) -> Result<Detection> {
        let Ok(meta) = std::fs::metadata(path) else {
            return analyze_file(path, hints);
        };
        let key = CacheKey { path: path.to_path_buf(), size: meta.len(), modified: meta.modified().ok(), hints: hints.clone() };
        if let Some(found) = self.entries.lock().expect("lock").get(&key) {
            self.hits.fetch_add(1, Ordering::SeqCst);
            return found.clone().map_err(CoreError::Analysis);
        }
        let result = analyze_file(path, hints);
        let kept = result.as_ref().map(Detection::clone).map_err(CoreError::to_string);
        self.entries.lock().expect("lock").insert(key, kept);
        result
    }

    /// Number of files kept (detections and failures).
    pub fn len(&self) -> usize {
        self.entries.lock().expect("lock").len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// How many lookups were answered without decoding.
    pub fn hits(&self) -> usize {
        self.hits.load(Ordering::SeqCst)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::name_tags::parse_name;

    #[test]
    fn hints_from_path() {
        let dirs = vec!["Lo-Fi Pack".to_string(), "One Shots".to_string(), "Hi-Hats".to_string()];
        let stem = "Closed Hat01 Dm";
        let hints = NameHints::from_path(&dirs, stem, &parse_name(stem));
        assert_eq!(hints.words, ["lo", "fi", "pack", "oneshot", "hi", "hats", "closed", "hat", "01", "dm"]);
        assert!(hints.has_key_token && !hints.has_bpm_token);
        assert!(hints.has_word("one shot") && hints.has_word("One-Shot") && hints.has_word("hat"));
        assert!(!hints.has_word("hi-hat"));
        assert!(NameHints::from_path(&[], "808bass", &parse_name("808bass")).has_word("808"));
    }

    #[test]
    fn one_shot_folds_only_whole_words() {
        let words = |text: &str| NameHints::from_path(&[], text, &NameTokens::default()).words;
        assert_eq!(words("phone shot"), ["phone", "shot"]);
        assert_eq!(words("someone shots"), ["someone", "shots"]);
        assert_eq!(words("One-Shots_kick"), ["oneshot", "kick"]);
        assert_eq!(words("one shot"), ["oneshot"]);
        assert_eq!(words("OneShots"), ["oneshot"]);
        assert_eq!(words("one"), ["one"]);
    }
}

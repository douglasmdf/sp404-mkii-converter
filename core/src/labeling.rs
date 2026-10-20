//! Merges what the analyzer heard with what the file name already says, and decides the
//! prefix: name values win, bare name tokens need the sound to agree, and only the fields
//! the sample kind uses end up in the prefix.

use serde::{Deserialize, Serialize};

use crate::analyze::{Detection, Estimate, ANALYZER_VERSION};
use crate::config::BpmRange;
use crate::error::CoreError;
use crate::name_tags::{NameTokens, Span};
use crate::tags::{self, fold_bpm, Chord, ChordQuality, Key, PitchClass, SampleKind};

/// A bare number in the name counts as the BPM when the detected raw BPM is this close
/// (relative) to it, to its half or to its double.
const BARE_BPM_TOLERANCE: f32 = 0.03;

/// The result shown in the Dry Run, the summary and the JSON log, and used for the name.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Analysis {
    pub kind: SampleKind,
    pub label: Option<String>,
    pub bpm: Option<u32>,
    pub kind_uncertain: bool,
    pub label_uncertain: bool,
    pub bpm_uncertain: bool,
    pub label_from_name: bool,
    pub bpm_from_name: bool,
    pub prefix: Option<String>,
    /// Why the file could not be analysed, as text (CLI, JSON log).
    pub error: Option<String>,
    /// The same reason as a code the app translates. Absent in plans from before it existed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error_kind: Option<AnalysisErrorKind>,
    pub analyzer_version: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AnalysisErrorKind {
    /// Empty or silent.
    NoAudio,
    /// The analyzer panicked on this file.
    Crashed,
    /// Could not be opened or decoded.
    Unreadable,
}

impl AnalysisErrorKind {
    pub fn of(error: &CoreError) -> AnalysisErrorKind {
        match error {
            CoreError::NoAudio => AnalysisErrorKind::NoAudio,
            _ => AnalysisErrorKind::Unreadable,
        }
    }
}

/// The analysis of a file that could not be analysed: no prefix, the reason in `error` and
/// `error_kind`.
pub fn failed(error: &CoreError) -> Analysis {
    failed_with(AnalysisErrorKind::of(error), &error.to_string())
}

/// The analysis of a file whose analysis panicked.
pub fn crashed() -> Analysis {
    failed_with(AnalysisErrorKind::Crashed, "analysis crashed")
}

fn failed_with(kind: AnalysisErrorKind, error: &str) -> Analysis {
    Analysis {
        kind: SampleKind::Unpitched,
        label: None,
        bpm: None,
        kind_uncertain: false,
        label_uncertain: false,
        bpm_uncertain: false,
        label_from_name: false,
        bpm_from_name: false,
        prefix: None,
        error: Some(error.to_string()),
        error_kind: Some(kind),
        analyzer_version: ANALYZER_VERSION,
    }
}

/// A chosen value: where it came from, whether it is trusted, and the name spans it used.
struct Pick<T> {
    value: T,
    from_name: bool,
    uncertain: bool,
    spans: Vec<Span>,
}

impl<T> Pick<T> {
    fn named(value: T, spans: Vec<Span>) -> Pick<T> {
        Pick { value, from_name: true, uncertain: false, spans }
    }

    fn detected<U>(estimate: &Estimate<U>, f: impl FnOnce(&U) -> T) -> Pick<T> {
        Pick { value: f(&estimate.value), from_name: false, uncertain: !estimate.confident, spans: Vec::new() }
    }
}

/// Picks the label and BPM for the detected kind, preferring the name's values, and returns
/// the spans of the name tokens that were used (to be removed from the name).
pub fn resolve(detection: &Detection, tokens: &NameTokens, range: BpmRange) -> (Analysis, Vec<Span>) {
    let kind = detection.kind.value;
    let label = match kind {
        SampleKind::Unpitched | SampleKind::DrumLoop => None,
        SampleKind::Note => note_label(detection, tokens),
        SampleKind::Chord => chord_label(detection, tokens),
        SampleKind::Phrase | SampleKind::MelodicLoop => key_label(detection, tokens),
    };
    let bpm = match kind {
        SampleKind::MelodicLoop | SampleKind::DrumLoop => bpm_value(detection, tokens, range),
        _ => None,
    };
    let mut spans = Vec::new();
    if let Some(p) = &label {
        spans.extend(&p.spans);
    }
    if let Some(p) = &bpm {
        spans.extend(&p.spans);
    }
    let label_text = label.as_ref().map(|p| p.value.clone());
    let bpm_value = bpm.as_ref().map(|p| p.value);
    let analysis = Analysis {
        kind,
        prefix: tags::prefix(label_text.as_deref(), bpm_value),
        label: label_text,
        bpm: bpm_value,
        kind_uncertain: !detection.kind.confident,
        label_uncertain: label.as_ref().is_some_and(|p| p.uncertain),
        bpm_uncertain: bpm.as_ref().is_some_and(|p| p.uncertain),
        label_from_name: label.as_ref().is_some_and(|p| p.from_name),
        bpm_from_name: bpm.as_ref().is_some_and(|p| p.from_name),
        error: None,
        error_kind: None,
        analyzer_version: ANALYZER_VERSION,
    };
    (analysis, spans)
}

/// A marked key gives its tonic; a bare letter or bare key (`Ab`) counts when it is the
/// detected note.
fn note_label(detection: &Detection, tokens: &NameTokens) -> Option<Pick<String>> {
    if let Some((key, spans)) = &tokens.key {
        return Some(Pick::named(key.tonic.name().to_string(), spans.clone()));
    }
    let detected = detection.note.as_ref()?;
    if let Some((letter, span)) = tokens.bare_letter.filter(|(l, _)| *l == detected.value) {
        return Some(Pick::named(letter.name().to_string(), vec![span]));
    }
    if let Some((key, span)) = tokens.bare_key.filter(|(k, _)| k.tonic == detected.value) {
        return Some(Pick::named(key.tonic.name().to_string(), vec![span]));
    }
    Some(Pick::detected(detected, |n: &PitchClass| n.name().to_string()))
}

/// A marked chord as written, or a marked key as its triad (`F#m` → chord `Gbm`); a bare key
/// (`Am`) only when it is the detected triad.
fn chord_label(detection: &Detection, tokens: &NameTokens) -> Option<Pick<String>> {
    if let Some((chord, spans)) = &tokens.chord {
        return Some(Pick::named(chord.label(), spans.clone()));
    }
    if let Some((key, spans)) = &tokens.key {
        return Some(Pick::named(triad(key).label(), spans.clone()));
    }
    let detected = detection.chord.as_ref()?;
    if let Some((key, span)) = tokens.bare_key.filter(|(k, _)| triad(k) == detected.value) {
        return Some(Pick::named(triad(&key).label(), vec![span]));
    }
    Some(Pick::detected(detected, Chord::label))
}

fn triad(key: &Key) -> Chord {
    let quality = if key.minor { ChordQuality::Minor } else { ChordQuality::Major };
    Chord { root: key.tonic, quality }
}

/// A marked key; or a bare key (`Am`) that is the detected key or its relative (label = the
/// name's key); or a bare letter that is the tonic of the detected key (label = that key)
/// or of its relative (label = the relative); else the detected key.
fn key_label(detection: &Detection, tokens: &NameTokens) -> Option<Pick<String>> {
    if let Some((key, spans)) = &tokens.key {
        return Some(Pick::named(key.label(), spans.clone()));
    }
    let detected = detection.key.as_ref()?;
    if let Some((key, span)) = tokens.bare_key.filter(|(k, _)| *k == detected.value || *k == detected.value.relative()) {
        return Some(Pick::named(key.label(), vec![span]));
    }
    if let Some((letter, span)) = tokens.bare_letter {
        let matching = [detected.value, detected.value.relative()].into_iter().find(|k: &Key| k.tonic == letter);
        if let Some(key) = matching {
            return Some(Pick::named(key.label(), vec![span]));
        }
    }
    Some(Pick::detected(detected, Key::label))
}

/// A marked BPM; or a bare number the detected raw BPM agrees with (±3 %, also at half or
/// double); else the detected BPM. Always folded into `range`.
fn bpm_value(detection: &Detection, tokens: &NameTokens, range: BpmRange) -> Option<Pick<u32>> {
    if let Some((bpm, spans)) = &tokens.bpm {
        return Some(Pick::named(fold_bpm(*bpm, range), spans.clone()));
    }
    let detected = detection.bpm.as_ref()?;
    if let Some((bare, span)) = tokens.bare_bpm {
        let agrees = [bare, bare / 2.0, bare * 2.0].iter().any(|&b| (detected.value - b).abs() <= BARE_BPM_TOLERANCE * b);
        if agrees {
            return Some(Pick::named(fold_bpm(bare, range), vec![span]));
        }
    }
    let mut pick = Pick::detected(detected, |&b: &f32| fold_bpm(b, range));
    // The analyzer finds the tempo family, not the octave: when double or half the value is
    // also inside the range (78–166 holds both 81 and 162), the choice is a guess.
    let (min, max) = range.bounds();
    let folded = pick.value as f32;
    pick.uncertain |= folded * 2.0 <= max || folded / 2.0 >= min;
    Some(pick)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::name_tags::parse_name;

    const RANGE: BpmRange = BpmRange::R78_166;

    #[test]
    fn detected_bpm_with_both_octaves_in_range_is_uncertain() {
        let detection = |bpm: f32| Detection {
            kind: Estimate { value: SampleKind::DrumLoop, confident: true },
            note: None,
            chord: None,
            key: None,
            bpm: sure(bpm),
        };
        let none = NameTokens::default();
        let (a, _) = resolve(&detection(81.0), &none, RANGE);
        assert_eq!((a.bpm, a.bpm_uncertain), (Some(81), true));
        let (a, _) = resolve(&detection(90.0), &none, RANGE);
        assert_eq!((a.bpm, a.bpm_uncertain), (Some(90), false));
        let (a, _) = resolve(&detection(81.0), &parse_name("Break 162bpm"), RANGE);
        assert_eq!((a.bpm, a.bpm_uncertain), (Some(162), false));
    }

    fn sure<T>(value: T) -> Option<Estimate<T>> {
        Some(Estimate { value, confident: true })
    }

    fn key(name: &str) -> Key {
        let minor = name.ends_with('m');
        let root = name.trim_end_matches('m');
        let mut chars = root.chars();
        let letter = chars.next().unwrap();
        let acc = match chars.next() {
            Some('b') => -1,
            Some('#') => 1,
            _ => 0,
        };
        Key { tonic: PitchClass::from_letter(letter, acc), minor }
    }

    fn detection(kind: SampleKind) -> Detection {
        Detection { kind: Estimate { value: kind, confident: true }, note: None, chord: None, key: None, bpm: None }
    }

    fn loop_detection(kind: SampleKind, k: &str, bpm: f32) -> Detection {
        Detection { key: sure(key(k)), bpm: sure(bpm), ..detection(kind) }
    }

    fn prefix_of(d: &Detection, stem: &str) -> Option<String> {
        resolve(d, &parse_name(stem), RANGE).0.prefix
    }

    #[test]
    fn resolve_uses_fields_of_kind() {
        let melodic = loop_detection(SampleKind::MelodicLoop, "Am", 90.2);
        assert_eq!(prefix_of(&melodic, "Loop").as_deref(), Some("Am_90"));
        let drums = loop_detection(SampleKind::DrumLoop, "Am", 90.2);
        assert_eq!(prefix_of(&drums, "Loop").as_deref(), Some("90"));
        let (analysis, spans) = resolve(&detection(SampleKind::Unpitched), &parse_name("Kick C#"), RANGE);
        assert_eq!(analysis.prefix, None);
        assert!(spans.is_empty());
        let note = Detection { note: sure(PitchClass(0)), ..detection(SampleKind::Note) };
        assert_eq!(prefix_of(&note, "808").as_deref(), Some("C"));
        let chord = Detection { chord: sure(Chord { root: PitchClass(0), quality: ChordQuality::Minor7 }), ..detection(SampleKind::Chord) };
        assert_eq!(prefix_of(&chord, "Stab 90bpm").as_deref(), Some("Cm7"));
        let phrase = Detection { key: sure(key("Am")), ..detection(SampleKind::Phrase) };
        assert_eq!(prefix_of(&phrase, "Arp").as_deref(), Some("Am"));
    }

    #[test]
    fn name_values_win() {
        let d = loop_detection(SampleKind::MelodicLoop, "Am", 92.0);
        let stem = "Keys F#m 140 BPM";
        let tokens = parse_name(stem);
        let (analysis, spans) = resolve(&d, &tokens, RANGE);
        assert_eq!(analysis.prefix.as_deref(), Some("Gbm_140"));
        assert!(analysis.label_from_name && analysis.bpm_from_name);
        assert!(!analysis.label_uncertain && !analysis.bpm_uncertain);
        assert_eq!(crate::name_tags::remove_spans(stem, &spans), "Keys");
    }

    #[test]
    fn marked_key_gives_note_and_chord() {
        let note = Detection { note: sure(PitchClass(5)), ..detection(SampleKind::Note) };
        assert_eq!(prefix_of(&note, "808 C#").as_deref(), Some("Db"));
        let chord = Detection { chord: sure(Chord { root: PitchClass(0), quality: ChordQuality::Major }), ..detection(SampleKind::Chord) };
        assert_eq!(prefix_of(&chord, "Stab A min").as_deref(), Some("Am"));
        assert_eq!(prefix_of(&chord, "Stab Cmaj7").as_deref(), Some("Cmaj7"));
    }

    #[test]
    fn values_the_kind_does_not_use_stay_in_the_name() {
        let d = loop_detection(SampleKind::DrumLoop, "C", 92.0);
        let stem = "Break Am 92bpm";
        let (analysis, spans) = resolve(&d, &parse_name(stem), RANGE);
        assert_eq!(analysis.prefix.as_deref(), Some("92"));
        assert_eq!(crate::name_tags::remove_spans(stem, &spans), "Break Am");
    }

    #[test]
    fn bare_tokens_need_audio_agreement() {
        let bpm_of = |detected: f32| {
            let (a, spans) = resolve(&loop_detection(SampleKind::DrumLoop, "C", detected), &parse_name("Take 92"), RANGE);
            (a.bpm, a.bpm_from_name, spans.len())
        };
        assert_eq!(bpm_of(46.1), (Some(92), true, 1));
        assert_eq!(bpm_of(120.0), (Some(120), false, 0));

        let label_of = |detected: &str| {
            let d = loop_detection(SampleKind::MelodicLoop, detected, 90.0);
            let (a, spans) = resolve(&d, &parse_name("Take A"), RANGE);
            (a.label, a.label_from_name, spans.len())
        };
        assert_eq!(label_of("C"), (Some("Am".to_string()), true, 1));
        assert_eq!(label_of("Am"), (Some("Am".to_string()), true, 1));
        assert_eq!(label_of("Eb"), (Some("Eb".to_string()), false, 0));
    }

    #[test]
    fn bare_am_and_ab_need_audio_agreement() {
        let label = |d: &Detection, stem: &str| {
            let (a, spans) = resolve(d, &parse_name(stem), RANGE);
            (a.label, a.label_from_name, crate::name_tags::remove_spans(stem, &spans))
        };
        let am = loop_detection(SampleKind::MelodicLoop, "Am", 90.0);
        assert_eq!(label(&am, "Loop Am 90bpm"), (Some("Am".to_string()), true, "Loop".to_string()));
        // The relative major agrees too; the name's spelling of the key is kept.
        let c = loop_detection(SampleKind::MelodicLoop, "C", 90.0);
        assert_eq!(label(&c, "Loop Am 90bpm"), (Some("Am".to_string()), true, "Loop".to_string()));
        // Otherwise the word is just a word.
        let eb = loop_detection(SampleKind::MelodicLoop, "Eb", 90.0);
        assert_eq!(label(&eb, "I Am Legend 90bpm"), (Some("Eb".to_string()), false, "I Am Legend".to_string()));
        assert_eq!(label(&eb, "Ab Workout Loop"), (Some("Eb".to_string()), false, "Ab Workout Loop".to_string()));
        let ab = loop_detection(SampleKind::MelodicLoop, "Ab", 90.0);
        assert_eq!(label(&ab, "Ab Workout Loop"), (Some("Ab".to_string()), true, "Workout Loop".to_string()));

        let note = |n: u8| Detection { note: sure(PitchClass(n)), ..detection(SampleKind::Note) };
        assert_eq!(prefix_of(&note(8), "Ab Bass").as_deref(), Some("Ab"));
        assert_eq!(label(&note(0), "Ab Bass").2, "Ab Bass");
        let chord = |root: u8, quality| Detection { chord: sure(Chord { root: PitchClass(root), quality }), ..detection(SampleKind::Chord) };
        assert_eq!(prefix_of(&chord(9, ChordQuality::Minor), "Stab Am").as_deref(), Some("Am"));
        assert_eq!(label(&chord(0, ChordQuality::Major), "Stab Am"), (Some("C".to_string()), false, "Stab Am".to_string()));
    }

    #[test]
    fn name_bpm_is_folded() {
        let d = loop_detection(SampleKind::DrumLoop, "C", 96.0);
        let (analysis, _) = resolve(&d, &parse_name("Breaks_Squash ME (192bpm)"), RANGE);
        assert_eq!(analysis.bpm, Some(96));
        assert_eq!(analysis.prefix.as_deref(), Some("96"));
    }

    #[test]
    fn uncertain_flags_follow_estimates() {
        let mut d = loop_detection(SampleKind::MelodicLoop, "Am", 90.0);
        d.key = Some(Estimate { value: key("Am"), confident: false });
        d.kind.confident = false;
        let (analysis, _) = resolve(&d, &parse_name("Loop"), RANGE);
        assert!(analysis.label_uncertain && analysis.kind_uncertain && !analysis.bpm_uncertain);
        assert_eq!(analysis.analyzer_version, ANALYZER_VERSION);
        assert_eq!(analysis.error, None);
    }

    #[test]
    fn failed_has_no_prefix() {
        let analysis = failed(&CoreError::Decode("bad header".into()));
        assert_eq!(analysis.error_kind, Some(AnalysisErrorKind::Unreadable));
        assert_eq!(analysis.kind, SampleKind::Unpitched);
        assert_eq!(analysis.prefix, None);
        assert_eq!(analysis.error.as_deref(), Some("could not decode audio: bad header"));
        assert_eq!(analysis.analyzer_version, ANALYZER_VERSION);
    }

    #[test]
    fn analysis_json_is_camel_case() {
        let json = serde_json::to_string(&failed(&CoreError::NoAudio)).unwrap();
        let version = format!("\"analyzerVersion\":{ANALYZER_VERSION}");
        assert!(json.contains(&version) && json.contains("\"kindUncertain\":false"), "{json}");
        assert!(json.contains("\"errorKind\":\"noAudio\"") && json.contains("\"error\":\"the file has no audible sound\""), "{json}");
        let crashed = serde_json::to_string(&crashed()).unwrap();
        assert!(crashed.contains("\"errorKind\":\"crashed\""), "{crashed}");
        // Plans written before the reason existed still load.
        let mut value: serde_json::Value = serde_json::from_str(&json).unwrap();
        value.as_object_mut().unwrap().remove("errorKind");
        let old: Analysis = serde_json::from_value(value).unwrap();
        assert_eq!(old.error_kind, None);
    }
}

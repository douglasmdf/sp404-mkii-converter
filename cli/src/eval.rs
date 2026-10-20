//! Accuracy of the key/BPM analyzer against a CSV answer key (`path,kind,label,bpm`).

use std::fmt::Write as _;

use sp404_core::config::BpmRange;
use sp404_core::labeling::Analysis;
use sp404_core::tags::{fold_bpm, Chord, ChordQuality, Key, PitchClass, SampleKind};

pub const KINDS: [SampleKind; 6] = [
    SampleKind::Unpitched,
    SampleKind::Note,
    SampleKind::Chord,
    SampleKind::Phrase,
    SampleKind::MelodicLoop,
    SampleKind::DrumLoop,
];

pub fn kind_name(kind: SampleKind) -> &'static str {
    match kind {
        SampleKind::Unpitched => "unpitched",
        SampleKind::Note => "note",
        SampleKind::Chord => "chord",
        SampleKind::Phrase => "phrase",
        SampleKind::MelodicLoop => "melodicLoop",
        SampleKind::DrumLoop => "drumLoop",
    }
}

fn kind_from_name(name: &str) -> Option<SampleKind> {
    KINDS.iter().copied().find(|k| kind_name(*k) == name)
}

/// One row of the answer key that is to be evaluated.
#[derive(Debug, Clone, PartialEq)]
pub struct Label {
    pub path: String,
    pub kind: SampleKind,
    pub label: Option<String>,
    pub bpm: Option<f32>,
}

/// `(hits, total)`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Ratio {
    pub hits: usize,
    pub total: usize,
}

impl Ratio {
    fn add(&mut self, hit: bool) {
        self.total += 1;
        self.hits += hit as usize;
    }

    fn percent(self) -> String {
        if self.total == 0 {
            String::from("-")
        } else {
            format!("{:.1}%", 100.0 * self.hits as f64 / self.total as f64)
        }
    }

    fn show(self) -> String {
        format!("{}/{} ({})", self.hits, self.total, self.percent())
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Metrics {
    pub kind: Ratio,
    /// `confusion[truth][result]`, indexed like `KINDS`.
    pub confusion: [[usize; 6]; 6],
    pub bpm_exact: Ratio,
    /// BPM wrong by exactly half or double (counted among the BPM rows).
    pub bpm_octave_errors: Ratio,
    pub key_exact: Ratio,
    pub key_exact_or_relative: Ratio,
    /// Weighted MIREX score: sum of weights over the number of key rows.
    pub mirex_sum: f64,
    pub mirex_total: usize,
    pub note_exact: Ratio,
    pub chord_exact: Ratio,
    pub chord_root: Ratio,
    pub drum_clean: Ratio,
    /// `(errors flagged uncertain, errors)` over every measure above that can be wrong.
    pub errors_flagged: Ratio,
    pub mean_millis: Option<f64>,
}

fn pitch_of(text: &str) -> Option<(PitchClass, &str)> {
    let mut chars = text.chars();
    let letter = chars.next().filter(|c| ('A'..='G').contains(c))?;
    let rest = chars.as_str();
    let (accidental, rest) = match rest.chars().next() {
        Some('#') => (1, &rest[1..]),
        Some('b') => (-1, &rest[1..]),
        _ => (0, rest),
    };
    Some((PitchClass::from_letter(letter, accidental), rest))
}

pub fn parse_pitch(text: &str) -> Option<PitchClass> {
    match pitch_of(text.trim())? {
        (pc, "") => Some(pc),
        _ => None,
    }
}

pub fn parse_key(text: &str) -> Option<Key> {
    let (tonic, rest) = pitch_of(text.trim())?;
    let minor = match rest {
        "" | "M" | "maj" | "major" => false,
        "m" | "min" | "minor" => true,
        _ => return None,
    };
    Some(Key { tonic, minor })
}

pub fn parse_chord(text: &str) -> Option<Chord> {
    let (root, rest) = pitch_of(text.trim())?;
    let quality = match rest {
        "" => ChordQuality::Major,
        "m" => ChordQuality::Minor,
        "7" => ChordQuality::Dominant7,
        "maj7" => ChordQuality::Major7,
        "m7" => ChordQuality::Minor7,
        "dim" => ChordQuality::Diminished,
        _ => return None,
    };
    Some(Chord { root, quality })
}

/// MIREX key score: exact 1, perfect fifth 0.5, relative 0.3, parallel 0.2, else 0.
pub fn mirex_weights(truth: Key, found: Key) -> f64 {
    if truth == found {
        return 1.0;
    }
    if truth.minor == found.minor {
        let diff = (found.tonic.0 + 12 - truth.tonic.0) % 12;
        return if diff == 7 || diff == 5 { 0.5 } else { 0.0 };
    }
    if found == truth.relative() {
        0.3
    } else if found.tonic == truth.tonic {
        0.2
    } else {
        0.0
    }
}

fn split_csv_line(line: &str) -> Vec<String> {
    let mut fields = Vec::new();
    let mut current = String::new();
    let mut quoted = false;
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '"' if quoted && chars.peek() == Some(&'"') => {
                current.push('"');
                chars.next();
            }
            '"' => quoted = !quoted,
            ',' if !quoted => fields.push(std::mem::take(&mut current)),
            _ => current.push(c),
        }
    }
    fields.push(current);
    fields.into_iter().map(|f| f.trim().to_string()).collect()
}

/// Parses the answer key. Rows with an empty `kind` are skipped (not evaluated). Columns after
/// the first four (e.g. `source`) are allowed and ignored.
pub fn parse_labels(text: &str) -> Result<Vec<Label>, String> {
    let mut lines = text.trim_start_matches('\u{feff}').lines().enumerate().filter(|(_, l)| !l.trim().is_empty());
    let Some((_, header)) = lines.next() else {
        return Err(String::from("the answer key is empty (expected the header path,kind,label,bpm)"));
    };
    let header = split_csv_line(header);
    if header.len() < 4 || header[..4] != ["path", "kind", "label", "bpm"] {
        return Err(String::from("the answer key must start with the header path,kind,label,bpm"));
    }
    let mut out = Vec::new();
    for (i, line) in lines {
        let n = i + 1;
        let fields = split_csv_line(line);
        if fields.len() != header.len() {
            return Err(format!("line {n}: expected {} columns, found {}", header.len(), fields.len()));
        }
        if fields[1].is_empty() {
            continue;
        }
        let kind = kind_from_name(&fields[1]).ok_or_else(|| {
            format!("line {n}: unknown kind \"{}\" (use unpitched, note, chord, phrase, melodicLoop or drumLoop)", fields[1])
        })?;
        let label = (!fields[2].is_empty()).then(|| fields[2].clone());
        if let Some(text) = &label {
            let valid = match kind {
                SampleKind::Note => parse_pitch(text).is_some(),
                SampleKind::Chord => parse_chord(text).is_some(),
                SampleKind::Phrase | SampleKind::MelodicLoop => parse_key(text).is_some(),
                _ => true,
            };
            if !valid {
                return Err(format!("line {n}: cannot read the label \"{text}\" for kind {}", fields[1]));
            }
        }
        let bpm = if fields[3].is_empty() {
            None
        } else {
            Some(fields[3].parse::<f32>().map_err(|_| format!("line {n}: cannot read the BPM \"{}\"", fields[3]))?)
        };
        out.push(Label { path: fields[0].clone(), kind, label, bpm });
    }
    Ok(out)
}

fn has_key(kind: SampleKind) -> bool {
    matches!(kind, SampleKind::Phrase | SampleKind::MelodicLoop)
}

fn has_bpm(kind: SampleKind) -> bool {
    matches!(kind, SampleKind::MelodicLoop | SampleKind::DrumLoop)
}

/// BPMs are compared after folding both the truth and the result into `range`.
pub fn score(results: &[(Label, Analysis)], range: BpmRange) -> Metrics {
    let mut m = Metrics::default();
    for (truth, found) in results {
        let ok = found.error.is_none();
        let kind_hit = ok && found.kind == truth.kind;
        m.kind.add(kind_hit);
        let row = KINDS.iter().position(|k| *k == truth.kind).unwrap_or(0);
        let col = KINDS.iter().position(|k| *k == found.kind).unwrap_or(0);
        m.confusion[row][col] += 1;
        let mut flag = |hit: bool, uncertain: bool| {
            if !hit {
                m.errors_flagged.total += 1;
                m.errors_flagged.hits += uncertain as usize;
            }
        };
        flag(kind_hit, found.kind_uncertain);

        if let (true, Some(want)) = (has_bpm(truth.kind), truth.bpm) {
            let want = fold_bpm(want, range) as f32;
            let got = found.bpm.filter(|_| ok).map(|b| fold_bpm(b as f32, range) as f32);
            let near = |a: f32, b: f32| (a - b).abs() <= 1.0;
            let exact = got.is_some_and(|g| near(g, want));
            m.bpm_exact.add(exact);
            m.bpm_octave_errors.add(!exact && got.is_some_and(|g| near(g * 2.0, want) || near(g / 2.0, want)));
            flag(exact, found.bpm_uncertain);
        }

        let found_label = found.label.as_deref().filter(|_| ok);
        if let (true, Some(text)) = (has_key(truth.kind), truth.label.as_deref()) {
            let want = parse_key(text);
            let got = found_label.filter(|_| has_key(found.kind)).and_then(parse_key);
            let weight = match (want, got) {
                (Some(w), Some(g)) => mirex_weights(w, g),
                _ => 0.0,
            };
            let exact = weight == 1.0;
            m.key_exact.add(exact);
            m.key_exact_or_relative.add(exact || weight == 0.3);
            m.mirex_sum += weight;
            m.mirex_total += 1;
            flag(exact, found.label_uncertain);
        }
        if let (SampleKind::Note, Some(text)) = (truth.kind, truth.label.as_deref()) {
            let hit = found.kind == SampleKind::Note && found_label.and_then(parse_pitch) == parse_pitch(text) && ok;
            m.note_exact.add(hit);
            flag(hit, found.label_uncertain);
        }
        if let (SampleKind::Chord, Some(text)) = (truth.kind, truth.label.as_deref()) {
            let want = parse_chord(text);
            let got = found_label.filter(|_| found.kind == SampleKind::Chord).and_then(parse_chord);
            let exact = want.is_some() && want == got;
            m.chord_exact.add(exact);
            m.chord_root.add(want.is_some() && want.map(|c| c.root) == got.map(|c| c.root));
            flag(exact, found.label_uncertain);
        }
        match truth.kind {
            SampleKind::Unpitched => m.drum_clean.add(ok && found.prefix.is_none() && found.label.is_none() && found.bpm.is_none()),
            SampleKind::DrumLoop => m.drum_clean.add(ok && found.label.is_none()),
            _ => {}
        }
    }
    m
}

impl Metrics {
    pub fn mirex(&self) -> Option<f64> {
        (self.mirex_total > 0).then(|| self.mirex_sum / self.mirex_total as f64)
    }

    /// Plain-text/Markdown tables of every measure.
    pub fn report(&self) -> String {
        let mut s = String::from("# Key/BPM accuracy\n\n| Measure | Result |\n|---|---|\n");
        let mirex = self.mirex().map_or(String::from("-"), |v| format!("{:.3} over {} loops/phrases", v, self.mirex_total));
        let time = self.mean_millis.map_or(String::from("-"), |v| format!("{v:.0} ms"));
        for (name, value) in [
            ("Type", self.kind.show()),
            ("BPM exact (+-1)", self.bpm_exact.show()),
            ("BPM half/double errors", self.bpm_octave_errors.show()),
            ("Key exact", self.key_exact.show()),
            ("Key exact or relative", self.key_exact_or_relative.show()),
            ("Key MIREX weighted score", mirex),
            ("Note exact", self.note_exact.show()),
            ("Chord exact", self.chord_exact.show()),
            ("Chord root only", self.chord_root.show()),
            ("Drums without key prefix", self.drum_clean.show()),
            ("Errors flagged uncertain", self.errors_flagged.show()),
            ("Mean time per file", time),
        ] {
            let _ = writeln!(s, "| {name} | {value} |");
        }
        s.push_str("\n## Type confusion (rows = truth, columns = result)\n\n| truth \\ result |");
        for k in KINDS {
            let _ = write!(s, " {} |", kind_name(k));
        }
        s.push_str("\n|---|---|---|---|---|---|---|\n");
        for (i, k) in KINDS.iter().enumerate() {
            let _ = write!(s, "| {} |", kind_name(*k));
            for n in self.confusion[i] {
                let _ = write!(s, " {n} |");
            }
            s.push('\n');
        }
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sp404_core::labeling::crashed;

    fn key(text: &str) -> Key {
        parse_key(text).unwrap()
    }

    fn analysis(kind: SampleKind, label: Option<&str>, bpm: Option<u32>) -> Analysis {
        Analysis { kind, label: label.map(String::from), bpm, error: None, error_kind: None, ..crashed() }
    }

    fn truth(kind: SampleKind, label: Option<&str>, bpm: Option<f32>) -> Label {
        Label { path: String::from("x.wav"), kind, label: label.map(String::from), bpm }
    }

    #[test]
    fn mirex_weights() {
        assert_eq!(super::mirex_weights(key("Am"), key("Am")), 1.0);
        assert_eq!(super::mirex_weights(key("Am"), key("Em")), 0.5);
        assert_eq!(super::mirex_weights(key("Am"), key("C")), 0.3);
        assert_eq!(super::mirex_weights(key("Am"), key("A")), 0.2);
        assert_eq!(super::mirex_weights(key("Am"), key("F#m")), 0.0);
    }

    #[test]
    fn bpm_octave_errors_counted() {
        let r = BpmRange::R78_166;
        let drum = |truth_bpm: f32, found: u32| {
            vec![(truth(SampleKind::DrumLoop, None, Some(truth_bpm)), analysis(SampleKind::DrumLoop, None, Some(found)))]
        };
        // 192 folds to 96: exact, no octave error.
        let m = score(&drum(192.0, 96), r);
        assert_eq!((m.bpm_exact.hits, m.bpm_octave_errors.hits), (1, 0));
        // 80 stays 80; 160 is double: an octave error.
        let m = score(&drum(80.0, 160), r);
        assert_eq!((m.bpm_exact.hits, m.bpm_octave_errors.hits), (0, 1));
    }

    #[test]
    fn parse_labels_rejects_bad_kind() {
        assert!(parse_labels("path,kind,label,bpm\na.wav,banana,,\n").is_err());
        assert!(parse_labels("a.wav,note,A,\n").is_err());
        let ok = parse_labels("path,kind,label,bpm\na.wav,note,F#,\nb.wav,,,\nc.wav,melodicLoop,Am,90\n").unwrap();
        assert_eq!(ok.len(), 2);
        assert_eq!(ok[0].label.as_deref(), Some("F#"));
        assert_eq!(ok[1].bpm, Some(90.0));
    }

    #[test]
    fn parse_labels_ignores_extra_columns() {
        let ok = parse_labels("path,kind,label,bpm,source\na.wav,melodicLoop,Am,90,reference\n").unwrap();
        assert_eq!(ok[0].label.as_deref(), Some("Am"));
        assert!(parse_labels("path,kind,label,bpm,source\na.wav,melodicLoop,Am,90\n").is_err());
        assert!(parse_labels("path,kind,bpm,label\n").is_err());
    }

    #[test]
    fn notes_chords_and_keys_are_scored() {
        let results = vec![
            (truth(SampleKind::Note, Some("F#"), None), analysis(SampleKind::Note, Some("Gb"), None)),
            (truth(SampleKind::Chord, Some("Cmaj7"), None), analysis(SampleKind::Chord, Some("C"), None)),
            (truth(SampleKind::MelodicLoop, Some("Am"), Some(90.0)), analysis(SampleKind::MelodicLoop, Some("C"), Some(91))),
        ];
        let m = score(&results, BpmRange::R78_166);
        assert_eq!(m.note_exact.hits, 1);
        assert_eq!((m.chord_exact.hits, m.chord_root.hits), (0, 1));
        assert_eq!((m.key_exact.hits, m.key_exact_or_relative.hits), (0, 1));
        assert_eq!(m.bpm_exact.hits, 1);
        assert!(m.report().contains("Key MIREX"));
    }
}

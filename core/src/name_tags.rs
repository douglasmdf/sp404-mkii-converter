//! Reads key, chord and BPM that are already written in a file name, so they can be
//! moved to the front as a prefix.

use crate::tags::{Chord, ChordQuality, Key, PitchClass};

/// Byte range inside the original stem.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

/// What was found in a stem. Marked tokens (`90bpm`, `F#m`, `Cmaj7`) are trusted; the
/// `bare_*` fields are only hints that need audio agreement before use. `Am` and `Ab` are
/// English words too ("I Am Legend", "Ab Workout"), so written exactly like that they are
/// only a `bare_key`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct NameTokens {
    pub key: Option<(Key, Vec<Span>)>,
    pub chord: Option<(Chord, Vec<Span>)>,
    pub bpm: Option<(f32, Vec<Span>)>,
    pub bare_bpm: Option<(f32, Span)>,
    pub bare_letter: Option<(PitchClass, Span)>,
    pub bare_key: Option<(Key, Span)>,
}

/// Key words that are also common English words.
const WORD_LIKE_KEYS: &[&str] = &["Am", "Ab"];

fn is_separator(c: char) -> bool {
    matches!(c, ' ' | '_' | '-' | '.' | ',' | '(' | ')' | '[' | ']')
}

struct Word<'a> {
    start: usize,
    end: usize,
    text: &'a str,
}

impl Word<'_> {
    fn span(&self) -> Span {
        Span { start: self.start, end: self.end }
    }
}

fn words(stem: &str) -> Vec<Word<'_>> {
    let mut out = Vec::new();
    let mut start: Option<usize> = None;
    for (i, c) in stem.char_indices() {
        if is_separator(c) {
            if let Some(s) = start.take() {
                out.push(Word { start: s, end: i, text: &stem[s..i] });
            }
        } else if start.is_none() {
            start = Some(i);
        }
    }
    if let Some(s) = start {
        out.push(Word { start: s, end: stem.len(), text: &stem[s..] });
    }
    out
}

fn all_digits(s: &str, min: usize, max: usize) -> Option<u32> {
    if (min..=max).contains(&s.len()) && s.bytes().all(|b| b.is_ascii_digit()) {
        s.parse().ok()
    } else {
        None
    }
}

/// Splits a leading `A-G` plus optional `#`/`b` off `s`.
fn split_root(s: &str) -> Option<(PitchClass, bool, &str)> {
    let mut chars = s.chars();
    let letter = chars.next().filter(|c| ('A'..='G').contains(c))?;
    let rest = chars.as_str();
    let (acc, rest) = match rest.chars().next() {
        Some('#') => (1, &rest[1..]),
        Some('b') => (-1, &rest[1..]),
        _ => (0, rest),
    };
    Some((PitchClass::from_letter(letter, acc), acc != 0, rest))
}

fn parse_chord(w: &str) -> Option<Chord> {
    let (root, _, rest) = split_root(w)?;
    let quality = match rest {
        "maj7" => ChordQuality::Major7,
        "m7" => ChordQuality::Minor7,
        "7" => ChordQuality::Dominant7,
        "dim" => ChordQuality::Diminished,
        _ => return None,
    };
    Some(Chord { root, quality })
}

fn is_major_word(s: &str) -> bool {
    s.eq_ignore_ascii_case("maj") || s.eq_ignore_ascii_case("major")
}

fn is_minor_word(s: &str) -> bool {
    s.eq_ignore_ascii_case("min") || s.eq_ignore_ascii_case("minor")
}

/// Single-word key: needs an accidental or a suffix (`m` only lowercase).
fn parse_key_word(w: &str) -> Option<Key> {
    let (tonic, has_accidental, rest) = split_root(w)?;
    let minor = if rest.is_empty() {
        if !has_accidental {
            return None;
        }
        false
    } else if rest == "m" || is_minor_word(rest) {
        true
    } else if is_major_word(rest) {
        false
    } else {
        return None;
    };
    Some(Key { tonic, minor })
}

/// Marked BPM in one word: `90bpm` or `bpm90`.
fn parse_bpm_word(w: &str) -> Option<f32> {
    let lower = w.to_ascii_lowercase();
    let digits = lower.strip_suffix("bpm").or_else(|| lower.strip_prefix("bpm"))?;
    all_digits(digits, 2, 3).map(|v| v as f32)
}

fn bpm_in_range(v: f32) -> bool {
    (40.0..=300.0).contains(&v)
}

pub fn parse_name(stem: &str) -> NameTokens {
    let ws = words(stem);
    let mut out = NameTokens::default();
    let mut used = vec![false; ws.len()];

    for i in 0..ws.len() {
        let w = &ws[i];

        if let Some(v) = parse_bpm_word(w.text).filter(|v| bpm_in_range(*v)) {
            out.bpm = Some((v, vec![w.span()]));
            used[i] = true;
            continue;
        }
        if let Some(n) = all_digits(w.text, 2, 3).map(|v| v as f32).filter(|v| bpm_in_range(*v)) {
            let next = ws.get(i + 1).filter(|x| x.text.eq_ignore_ascii_case("bpm") && !used[i + 1]);
            let prev = i
                .checked_sub(1)
                .filter(|p| !used[*p])
                .map(|p| &ws[p])
                .filter(|x| x.text.eq_ignore_ascii_case("bpm"));
            if let Some(b) = next {
                out.bpm = Some((n, vec![w.span(), b.span()]));
                used[i] = true;
                used[i + 1] = true;
                continue;
            }
            if let Some(b) = prev {
                out.bpm = Some((n, vec![b.span(), w.span()]));
                used[i] = true;
                used[i - 1] = true;
                continue;
            }
        }
        if let Some(chord) = parse_chord(w.text) {
            out.chord = Some((chord, vec![w.span()]));
            used[i] = true;
            continue;
        }
        if WORD_LIKE_KEYS.contains(&w.text) {
            // Left for the bare pass below.
            continue;
        }
        // A root (`A`, `A#`, `Bb`) followed by the word min/minor/maj/major: checked before the
        // single-word key, or `A# Min` would read as A# major and leave `Min` in the name.
        if let Some(next) = ws.get(i + 1).filter(|n| is_minor_word(n.text) || is_major_word(n.text)) {
            if let Some((tonic, _, "")) = split_root(w.text) {
                let key = Key { tonic, minor: is_minor_word(next.text) };
                out.key = Some((key, vec![w.span(), next.span()]));
                used[i] = true;
                used[i + 1] = true;
                continue;
            }
        }
        if let Some(key) = parse_key_word(w.text) {
            out.key = Some((key, vec![w.span()]));
            used[i] = true;
            continue;
        }
    }

    for (i, w) in ws.iter().enumerate() {
        if used[i] {
            continue;
        }
        if let Some(n) = all_digits(w.text, 2, 3).filter(|n| (50..=250).contains(n)) {
            out.bare_bpm = Some((n as f32, w.span()));
        } else if WORD_LIKE_KEYS.contains(&w.text) {
            out.bare_key = parse_key_word(w.text).map(|key| (key, w.span()));
        } else {
            let mut chars = w.text.chars();
            if let (Some(l), None) = (chars.next(), chars.next()) {
                if ('A'..='G').contains(&l) {
                    out.bare_letter = Some((PitchClass::from_letter(l, 0), w.span()));
                }
            }
        }
    }
    out
}

/// Deletes the spans together with the separators left over around them: empty `()`/`[]`
/// are dropped, a separator run touching a removed span collapses to its first character
/// and is trimmed at the ends. Separators elsewhere in the stem stay as they are.
pub fn remove_spans(stem: &str, spans: &[Span]) -> String {
    if spans.is_empty() {
        return stem.to_string();
    }
    const MARK: char = '\u{0}';
    let mut sorted: Vec<Span> = spans.to_vec();
    sorted.sort_by_key(|s| s.start);
    let mut chars: Vec<char> = Vec::with_capacity(stem.len());
    let mut pos = 0;
    for s in &sorted {
        if s.start > pos {
            chars.extend(stem[pos..s.start].chars());
        }
        if s.end > pos {
            chars.push(MARK);
            pos = s.end;
        }
    }
    if pos < stem.len() {
        chars.extend(stem[pos..].chars());
    }

    // Remove "()" / "[]" emptied by the removal, repeatedly (they can nest).
    loop {
        let mut removed = false;
        let mut i = 0;
        while i < chars.len() {
            let close = match chars[i] {
                '(' => ')',
                '[' => ']',
                _ => {
                    i += 1;
                    continue;
                }
            };
            let mut j = i + 1;
            let mut has_mark = false;
            while j < chars.len() && (chars[j] == ' ' || chars[j] == MARK) {
                has_mark |= chars[j] == MARK;
                j += 1;
            }
            if has_mark && j < chars.len() && chars[j] == close {
                chars.splice(i..=j, [MARK]);
                removed = true;
            } else {
                i += 1;
            }
        }
        if !removed {
            break;
        }
    }

    // Collapse each separator run that touches a removal point.
    let mut out = String::with_capacity(chars.len());
    let mut i = 0;
    while i < chars.len() {
        if is_separator(chars[i]) || chars[i] == MARK {
            let start = i;
            while i < chars.len() && (is_separator(chars[i]) || chars[i] == MARK) {
                i += 1;
            }
            let run = &chars[start..i];
            if run.contains(&MARK) {
                if start > 0 && i < chars.len() {
                    if let Some(c) = run.iter().find(|c| **c != MARK) {
                        out.push(*c);
                    }
                }
            } else {
                out.extend(run);
            }
        } else {
            out.push(chars[i]);
            i += 1;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn all_spans(t: &NameTokens) -> Vec<Span> {
        let mut v = Vec::new();
        if let Some((_, s)) = &t.key {
            v.extend(s);
        }
        if let Some((_, s)) = &t.chord {
            v.extend(s);
        }
        if let Some((_, s)) = &t.bpm {
            v.extend(s);
        }
        v
    }

    fn check(stem: &str, key: Option<&str>, chord: Option<&str>, bpm: Option<f32>, rest: &str) {
        let t = parse_name(stem);
        assert_eq!(t.key.as_ref().map(|(k, _)| k.label()).as_deref(), key, "key of {stem:?}");
        assert_eq!(t.chord.as_ref().map(|(c, _)| c.label()).as_deref(), chord, "chord of {stem:?}");
        assert_eq!(t.bpm.as_ref().map(|(b, _)| *b), bpm, "bpm of {stem:?}");
        assert_eq!(remove_spans(stem, &all_spans(&t)), rest, "rest of {stem:?}");
    }

    #[test]
    fn sharp_or_flat_root_followed_by_min_or_maj_word() {
        // Cymatics names: "A# Min", "D# Maj" — the minor/major word belongs to the key.
        for (stem, label) in [
            ("Cymatics - Angelic Tribe Vocal Loop - 109 BPM A# Min", "Bbm"),
            ("Cymatics - Belong Together Vocal Loop - 113 BPM D# Maj", "Eb"),
            ("Loop Bb minor", "Bbm"),
        ] {
            let t = parse_name(stem);
            assert_eq!(t.key.as_ref().map(|(k, _)| k.label()).as_deref(), Some(label), "{stem}");
            let rest = remove_spans(stem, &all_spans(&t));
            assert!(!rest.contains("Min") && !rest.contains("Maj") && !rest.contains("minor"), "{stem} -> {rest}");
        }
    }

    #[test]
    fn parses_spec_examples() {
        check("Loop_90bpm_A minor", Some("Am"), None, Some(90.0), "Loop");
        check("Keys F#m 140 BPM", Some("Gbm"), None, Some(140.0), "Keys");
        check("808 C#", Some("Db"), None, None, "808");
        check("Break 92bpm", None, None, Some(92.0), "Break");
        check("Breaks_Fly (90bpm)", None, None, Some(90.0), "Breaks_Fly");
        check("Snare_Tambo 1 (85bpm)", None, None, Some(85.0), "Snare_Tambo 1");
        check("Piano Cmaj7", None, Some("Cmaj7"), None, "Piano");
    }

    #[test]
    fn key_tokens_need_word_boundaries() {
        for stem in ["Amazing Pad", "Bbq Loop", "Dmx Hit", "I am here", "Cmajestic"] {
            let t = parse_name(stem);
            assert!(t.key.is_none(), "{stem:?}");
            assert!(t.chord.is_none(), "{stem:?}");
            assert!(t.bare_letter.is_none(), "{stem:?}");
        }
    }

    #[test]
    fn bare_tokens_are_reported_not_trusted() {
        assert!(parse_name("Kick 01").bare_bpm.is_none());
        let t = parse_name("Loop 92");
        assert_eq!(t.bare_bpm.map(|(v, _)| v), Some(92.0));
        assert!(t.bpm.is_none());
        let t = parse_name("Take A");
        assert_eq!(t.bare_letter.map(|(p, _)| p), Some(PitchClass::from_letter('A', 0)));
        assert!(t.key.is_none());
    }

    #[test]
    fn last_candidate_wins() {
        let t = parse_name("Loop 90bpm alt 92bpm");
        assert_eq!(t.bpm.map(|(v, _)| v), Some(92.0));
    }

    #[test]
    fn remove_spans_cleans_separators() {
        let stem = "Loop (90bpm) [Dm]";
        let t = parse_name(stem);
        assert_eq!(remove_spans(stem, &all_spans(&t)), "Loop");
        let whole = [Span { start: 0, end: 5 }];
        assert_eq!(remove_spans("90bpm", &whole), "");
    }

    #[test]
    fn remove_spans_keeps_untouched_separators() {
        let stem = "Kick - Hard 90bpm";
        let t = parse_name(stem);
        assert_eq!(remove_spans(stem, &all_spans(&t)), "Kick - Hard");
        assert_eq!(remove_spans("Kick - Hard", &[]), "Kick - Hard");
    }

    #[test]
    fn claimed_bpm_word_is_not_reused() {
        let t = parse_name("Loop 90 bpm 120");
        let (v, spans) = t.bpm.unwrap();
        assert_eq!(v, 90.0);
        assert!(spans.iter().all(|s| s.end <= 12));
        assert_eq!(t.bare_bpm.map(|(v, _)| v), Some(120.0));
    }

    #[test]
    fn more_parse_cases() {
        check("Loop bpm90", None, None, Some(90.0), "Loop");
        check("Loop BPM 140", None, None, Some(140.0), "Loop");
        check("Bb", Some("Bb"), None, None, "");
        check("Piano C#m7", None, Some("Dbm7"), None, "Piano");
        assert!(parse_name("Loop AM").key.is_none());
    }

    #[test]
    fn english_words_am_and_ab_are_only_bare_keys() {
        // "Am" and "Ab" are also English words: hints that need the audio to agree.
        let t = parse_name("I Am Legend 90bpm");
        assert!(t.key.is_none());
        assert_eq!(t.bpm.as_ref().map(|(b, _)| *b), Some(90.0));
        assert_eq!(t.bare_key.as_ref().map(|(k, _)| k.label()).as_deref(), Some("Am"));
        let t = parse_name("Ab Workout Loop");
        assert!(t.key.is_none());
        assert_eq!(t.bare_key.as_ref().map(|(k, _)| k.label()).as_deref(), Some("Ab"));
        // With a separate minor/major marker, or another spelling, the key stays marked.
        for (stem, label) in [("Loop A min", "Am"), ("Loop Amin", "Am"), ("Loop A minor", "Am"), ("Loop Abm", "Abm"), ("Loop G#", "Ab")] {
            let t = parse_name(stem);
            assert_eq!(t.key.as_ref().map(|(k, _)| k.label()).as_deref(), Some(label), "{stem:?}");
            assert!(t.bare_key.is_none(), "{stem:?}");
        }
    }
}

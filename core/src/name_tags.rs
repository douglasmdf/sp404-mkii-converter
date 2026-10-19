//! Reads key, chord and BPM that are already written in a file name, so they can be
//! moved to the front as a prefix.

use crate::tags::{Chord, ChordQuality, Key, PitchClass};

/// Byte range inside the original stem.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

/// What was found in a stem. Marked tokens (`90bpm`, `Am`, `Cmaj7`) are trusted; the
/// `bare_*` fields are only hints that need audio agreement before use.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct NameTokens {
    pub key: Option<(Key, Vec<Span>)>,
    pub chord: Option<(Chord, Vec<Span>)>,
    pub bpm: Option<(f32, Vec<Span>)>,
    pub bare_bpm: Option<(f32, Span)>,
    pub bare_letter: Option<(PitchClass, Span)>,
}

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
            let next = ws.get(i + 1).filter(|x| x.text.eq_ignore_ascii_case("bpm"));
            let prev = i.checked_sub(1).map(|p| &ws[p]).filter(|x| x.text.eq_ignore_ascii_case("bpm"));
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
        if let Some(key) = parse_key_word(w.text) {
            out.key = Some((key, vec![w.span()]));
            used[i] = true;
            continue;
        }
        // Letter followed by the word min/minor/maj/major.
        if let Some(next) = ws.get(i + 1) {
            let mut chars = w.text.chars();
            if let (Some(l), None) = (chars.next(), chars.next()) {
                if ('A'..='G').contains(&l) && (is_minor_word(next.text) || is_major_word(next.text)) {
                    let key = Key {
                        tonic: PitchClass::from_letter(l, 0),
                        minor: is_minor_word(next.text),
                    };
                    out.key = Some((key, vec![w.span(), next.span()]));
                    used[i] = true;
                    used[i + 1] = true;
                }
            }
        }
    }

    for (i, w) in ws.iter().enumerate() {
        if used[i] {
            continue;
        }
        if let Some(n) = all_digits(w.text, 2, 3).filter(|n| (50..=250).contains(n)) {
            out.bare_bpm = Some((n as f32, w.span()));
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

/// Deletes the spans, drops brackets left empty, collapses each run of separators to its
/// first character and trims separators from both ends.
pub fn remove_spans(stem: &str, spans: &[Span]) -> String {
    let mut sorted: Vec<Span> = spans.to_vec();
    sorted.sort_by_key(|s| s.start);
    let mut kept = String::with_capacity(stem.len());
    let mut pos = 0;
    for s in &sorted {
        if s.start > pos {
            kept.push_str(&stem[pos..s.start]);
        }
        pos = pos.max(s.end);
    }
    if pos < stem.len() {
        kept.push_str(&stem[pos..]);
    }

    let mut chars: Vec<char> = kept.chars().collect();
    // Remove "()" / "[]" that hold nothing but spaces, repeatedly (they can nest).
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
            while j < chars.len() && chars[j] == ' ' {
                j += 1;
            }
            if j < chars.len() && chars[j] == close {
                chars.drain(i..=j);
                removed = true;
            } else {
                i += 1;
            }
        }
        if !removed {
            break;
        }
    }

    let mut out = String::with_capacity(chars.len());
    let mut in_run = false;
    for c in chars {
        if is_separator(c) {
            if !in_run {
                out.push(c);
            }
            in_run = true;
        } else {
            out.push(c);
            in_run = false;
        }
    }
    out.trim_matches(is_separator).to_string()
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
        let stem = "Loop (90bpm) [Am]";
        let t = parse_name(stem);
        assert_eq!(remove_spans(stem, &all_spans(&t)), "Loop");
        let whole = [Span { start: 0, end: 5 }];
        assert_eq!(remove_spans("90bpm", &whole), "");
    }
}

//! Musical value types (key, chord, BPM) and the prefix they add to output names.

use serde::{Deserialize, Serialize};

use crate::config::BpmRange;

const NAMES: [&str; 12] = ["C", "Db", "D", "Eb", "E", "F", "Gb", "G", "Ab", "A", "Bb", "B"];

/// Pitch class, 0 = C ... 11 = B.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PitchClass(pub u8);

impl PitchClass {
    /// Flat spelling of the note.
    pub fn name(self) -> &'static str {
        NAMES[(self.0 % 12) as usize]
    }

    /// `letter` is `A`-`G`; `accidental` is +1 for `#`, -1 for `b`. Anything else maps to C.
    pub fn from_letter(letter: char, accidental: i8) -> PitchClass {
        let base: i32 = match letter.to_ascii_uppercase() {
            'C' => 0,
            'D' => 2,
            'E' => 4,
            'F' => 5,
            'G' => 7,
            'A' => 9,
            'B' => 11,
            _ => 0,
        };
        PitchClass((base + accidental as i32).rem_euclid(12) as u8)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Key {
    pub tonic: PitchClass,
    pub minor: bool,
}

impl Key {
    /// `Am` for A minor, `C` for C major.
    pub fn label(&self) -> String {
        format!("{}{}", self.tonic.name(), if self.minor { "m" } else { "" })
    }

    /// The relative major or minor key (Am <-> C).
    pub fn relative(&self) -> Key {
        if self.minor {
            Key { tonic: PitchClass((self.tonic.0 + 3) % 12), minor: false }
        } else {
            Key { tonic: PitchClass((self.tonic.0 + 9) % 12), minor: true }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ChordQuality {
    Major,
    Minor,
    Dominant7,
    Major7,
    Minor7,
    Diminished,
}

impl ChordQuality {
    pub fn suffix(self) -> &'static str {
        match self {
            ChordQuality::Major => "",
            ChordQuality::Minor => "m",
            ChordQuality::Dominant7 => "7",
            ChordQuality::Major7 => "maj7",
            ChordQuality::Minor7 => "m7",
            ChordQuality::Diminished => "dim",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Chord {
    pub root: PitchClass,
    pub quality: ChordQuality,
}

impl Chord {
    pub fn label(&self) -> String {
        format!("{}{}", self.root.name(), self.quality.suffix())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SampleKind {
    Unpitched,
    Note,
    Chord,
    Phrase,
    MelodicLoop,
    DrumLoop,
}

/// Doubles or halves `bpm` until it lies in `range`, then rounds.
pub fn fold_bpm(bpm: f32, range: BpmRange) -> u32 {
    let (min, max) = range.bounds();
    let mut v = bpm;
    if !(v.is_finite() && v > 0.0) {
        return 0;
    }
    while v < min {
        v *= 2.0;
    }
    while v > max {
        v /= 2.0;
    }
    v.round() as u32
}

/// `("Am", 90)` -> `Am_90`; either part may be missing.
pub fn prefix(label: Option<&str>, bpm: Option<u32>) -> Option<String> {
    match (label, bpm) {
        (Some(l), Some(b)) => Some(format!("{l}_{b}")),
        (Some(l), None) => Some(l.to_string()),
        (None, Some(b)) => Some(b.to_string()),
        (None, None) => None,
    }
}

/// Puts the prefix in front of the stem, separated by `_`.
pub fn prepend(prefix: Option<&str>, stem: &str) -> String {
    match prefix {
        None => stem.to_string(),
        Some(p) if stem.is_empty() => p.to_string(),
        Some(p) => format!("{p}_{stem}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::BpmRange;

    #[test]
    fn pitch_names_use_flats() {
        let names: Vec<&str> = (0..12).map(|i| PitchClass(i).name()).collect();
        assert_eq!(names, ["C", "Db", "D", "Eb", "E", "F", "Gb", "G", "Ab", "A", "Bb", "B"]);
        assert_eq!(PitchClass::from_letter('F', 1).name(), "Gb");
        assert_eq!(PitchClass::from_letter('C', -1).name(), "B");
    }

    #[test]
    fn key_and_chord_labels() {
        let a = PitchClass::from_letter('A', 0);
        let c = PitchClass::from_letter('C', 0);
        assert_eq!(Key { tonic: a, minor: true }.label(), "Am");
        assert_eq!(Key { tonic: c, minor: false }.label(), "C");
        assert_eq!(Key { tonic: a, minor: true }.relative().label(), "C");
        let got: Vec<String> = [
            ChordQuality::Major,
            ChordQuality::Minor,
            ChordQuality::Dominant7,
            ChordQuality::Major7,
            ChordQuality::Minor7,
            ChordQuality::Diminished,
        ]
        .iter()
        .map(|&quality| Chord { root: c, quality }.label())
        .collect();
        assert_eq!(got, ["C", "Cm", "C7", "Cmaj7", "Cm7", "Cdim"]);
    }

    #[test]
    fn fold_bpm_lands_in_range() {
        let r = BpmRange::R78_166;
        assert_eq!(fold_bpm(70.0, r), 140);
        assert_eq!(fold_bpm(192.0, r), 96);
        assert_eq!(fold_bpm(169.0, r), 85);
        assert_eq!(fold_bpm(90.4, r), 90);
        assert_eq!(fold_bpm(40.0, r), 80);
        assert_eq!(fold_bpm(162.0, BpmRange::R58_115), 81);
    }

    #[test]
    fn prefix_and_prepend() {
        assert_eq!(prefix(Some("Am"), Some(90)).as_deref(), Some("Am_90"));
        assert_eq!(prefix(None, Some(90)).as_deref(), Some("90"));
        assert_eq!(prefix(Some("C"), None).as_deref(), Some("C"));
        assert_eq!(prefix(None, None), None);
        assert_eq!(prepend(Some("Am_90"), "Loop"), "Am_90_Loop");
        assert_eq!(prepend(Some("Am_90"), ""), "Am_90");
        assert_eq!(prepend(None, "Loop"), "Loop");
        assert_eq!(prepend(None, ""), "");
    }
}

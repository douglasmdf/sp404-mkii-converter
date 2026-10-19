//! Chord of a one-shot and key of a loop, from the chromagram.

use super::features::Features;
use super::Estimate;
use crate::tags::{Chord, ChordQuality, Key, PitchClass};

const QUALITIES: [(ChordQuality, &[usize]); 6] = [
    (ChordQuality::Major, &[0, 4, 7]),
    (ChordQuality::Minor, &[0, 3, 7]),
    (ChordQuality::Dominant7, &[0, 4, 7, 10]),
    (ChordQuality::Major7, &[0, 4, 7, 11]),
    (ChordQuality::Minor7, &[0, 3, 7, 10]),
    (ChordQuality::Diminished, &[0, 3, 6]),
];
/// Each chord note also adds its first harmonics to the template: (pitch-class offset, weight)
/// for harmonics 1–4 at 1/h amplitude (octave, octave + fifth, two octaves).
const HARMONICS: [(usize, f32); 4] = [(0, 1.0), (0, 0.5), (7, 1.0 / 3.0), (0, 0.25)];
/// Bonus for a chord or key whose root/tonic carries the bass, × its share of the bass chroma.
const BASS_ROOT_WEIGHT: f32 = 0.1;
/// Bass chroma is used only when it holds at least this share of the full chroma's energy.
const BASS_PRESENCE: f32 = 0.02;
/// Best chord must beat the runner-up by this (cosine) to be confident.
const CHORD_MARGIN: f32 = 0.03;

/// Krumhansl-Kessler key profiles, tonic first.
const MAJOR_PROFILE: [f32; 12] = [6.35, 2.23, 3.48, 2.33, 4.38, 4.09, 2.52, 5.19, 2.39, 3.66, 2.29, 2.88];
const MINOR_PROFILE: [f32; 12] = [6.33, 2.68, 3.52, 5.38, 2.60, 3.53, 2.54, 4.75, 3.98, 2.69, 3.34, 3.17];
/// Relative keys closer than this (profile correlation) go to the tie-break. Diatonic loops
/// correlate almost as well with the relative key, so with this margin the tie-break, not the
/// profiles, decides most of them.
const RELATIVE_MARGIN: f32 = 0.25;
/// Tie-break weights: tonic in the bass, tonic at the start/end, tonic chord over the loop.
const TIE_BASS_WEIGHT: f32 = 1.0;
const TIE_ENDS_WEIGHT: f32 = 1.0;
const TIE_CHORD_WEIGHT: f32 = 1.0;
/// Length of the start and end windows for the tie-break.
const ENDS_SECONDS: f32 = 1.0;
/// Segment length for counting tonic chords when there are no onsets.
const SEGMENT_SECONDS: f32 = 0.5;
/// Profile margin needed to be confident: over the runner-up, or (after a tie-break) between
/// the relative pair and the best key outside it.
const KEY_MARGIN: f32 = 0.05;
/// After a tie-break, the winner's total must also beat its relative by this much. The
/// evidence spans 0–3 (three shares weighted 1.0), so this is 5 % of its range.
const TIE_BREAK_MARGIN: f32 = 0.15;
/// A pitch class is strong in a frame at this fraction of the frame's loudest class...
const POLY_STRONG: f32 = 0.5;
/// ...and stable when strong in at least this share of the sustained frames.
const POLY_STABLE_SHARE: f32 = 0.7;

/// Chord of a one-shot: the sustained chromagram against 72 chord templates, with the root
/// nudged toward the bass note.
pub fn estimate_chord(features: &Features) -> Option<Estimate<Chord>> {
    let range = features.sustained_range();
    let chroma = sum(&features.chroma[range.clone()]);
    let bass = bass_share(&features.bass_chroma[range], &chroma);
    let ranked = rank_chords(&chroma, &bass)?;
    let confident = ranked.len() < 2 || ranked[0].1 - ranked[1].1 >= CHORD_MARGIN;
    Some(Estimate { value: ranked[0].0, confident })
}

/// Key of a loop: mean chromagram correlated with 24 key profiles; a close call between
/// relative keys (Am vs C) is settled by the bass, the start/end and the tonic chord count.
pub fn estimate_key(features: &Features) -> Option<Estimate<Key>> {
    let chroma = sum(&features.chroma);
    if chroma.iter().sum::<f32>() <= 0.0 {
        return None;
    }
    let mut keys: Vec<(Key, f32)> = (0..24)
        .map(|i| {
            let key = Key { tonic: PitchClass((i % 12) as u8), minor: i >= 12 };
            (key, key_correlation(&chroma, key))
        })
        .collect();
    keys.sort_by(|a, b| b.1.total_cmp(&a.1));
    let (best, best_score) = keys[0];
    let relative = best.relative();
    let relative_score = keys.iter().find(|(k, _)| *k == relative).map_or(f32::MIN, |k| k.1);
    if best_score - relative_score >= RELATIVE_MARGIN {
        let margin = best_score - keys[1].1;
        return Some(Estimate { value: best, confident: margin >= KEY_MARGIN });
    }
    let total = |key: Key, score: f32| score + tie_break_evidence(features, &chroma, key);
    let (a, b) = (total(best, best_score), total(relative, relative_score));
    let value = if b > a { relative } else { best };
    // Any key outside the pair that is nearly as good as the pair's best makes the call uncertain.
    let rival = keys.iter().find(|(k, _)| *k != best && *k != relative).map_or(f32::MIN, |k| k.1);
    let confident = best_score - rival >= KEY_MARGIN && (a - b).abs() >= TIE_BREAK_MARGIN;
    Some(Estimate { value, confident })
}

/// Three or more pitch classes that stay strong together through the sustained part.
pub fn is_polyphonic(features: &Features) -> bool {
    let frames = &features.chroma[features.sustained_range()];
    if frames.is_empty() {
        return false;
    }
    let mut strong = [0usize; 12];
    for frame in frames {
        let loudest = frame.iter().fold(0f32, |m, &v| m.max(v));
        if loudest <= 0.0 {
            continue;
        }
        for (count, &v) in strong.iter_mut().zip(frame) {
            if v >= POLY_STRONG * loudest {
                *count += 1;
            }
        }
    }
    strong.iter().filter(|&&c| c as f32 >= POLY_STABLE_SHARE * frames.len() as f32).count() >= 3
}

/// Every chord with its score, best first; `None` for an empty chromagram.
fn rank_chords(chroma: &[f32; 12], bass: &[f32; 12]) -> Option<Vec<(Chord, f32)>> {
    let norm = l2(chroma);
    if norm <= 0.0 {
        return None;
    }
    let mut ranked: Vec<(Chord, f32)> = (0..12)
        .flat_map(|root| QUALITIES.iter().map(move |&(quality, intervals)| (root, quality, intervals)))
        .map(|(root, quality, intervals)| {
            let template = chord_template(root, intervals);
            let cosine = dot(chroma, &template) / (norm * l2(&template));
            (Chord { root: PitchClass(root as u8), quality }, cosine + BASS_ROOT_WEIGHT * bass[root])
        })
        .collect();
    ranked.sort_by(|a, b| b.1.total_cmp(&a.1));
    Some(ranked)
}

fn chord_template(root: usize, intervals: &[usize]) -> [f32; 12] {
    let mut template = [0f32; 12];
    for &interval in intervals {
        for &(offset, weight) in &HARMONICS {
            template[(root + interval + offset) % 12] += weight;
        }
    }
    template
}

/// Pearson correlation of the chromagram with the key's profile.
fn key_correlation(chroma: &[f32; 12], key: Key) -> f32 {
    let profile = if key.minor { &MINOR_PROFILE } else { &MAJOR_PROFILE };
    let rotated: [f32; 12] = std::array::from_fn(|pc| profile[(pc + 12 - key.tonic.0 as usize) % 12]);
    pearson(chroma, &rotated)
}

/// How much the bass, the start/end of the loop and its chords point at `key`'s tonic.
fn tie_break_evidence(features: &Features, chroma: &[f32; 12], key: Key) -> f32 {
    let tonic = key.tonic.0 as usize;
    let bass = bass_share(&features.bass_chroma, chroma)[tonic];

    let edge = ((ENDS_SECONDS / features.hop_seconds) as usize).min(features.chroma.len());
    let mut ends = sum(&features.chroma[..edge]);
    let tail = sum(&features.chroma[features.chroma.len() - edge..]);
    ends.iter_mut().zip(tail).for_each(|(e, t)| *e += t);
    let ends_total: f32 = ends.iter().sum();
    let ends_share = if ends_total > 0.0 { ends[tonic] / ends_total } else { 0.0 };

    let segments = segments(features);
    let tonic_chords = segments
        .iter()
        .filter(|range| {
            let seg_chroma = sum(&features.chroma[(*range).clone()]);
            let seg_bass = bass_share(&features.bass_chroma[(*range).clone()], &seg_chroma);
            rank_chords(&seg_chroma, &seg_bass).is_some_and(|r| is_tonic_chord(r[0].0, key))
        })
        .count();
    let chord_share = if segments.is_empty() { 0.0 } else { tonic_chords as f32 / segments.len() as f32 };

    TIE_BASS_WEIGHT * bass + TIE_ENDS_WEIGHT * ends_share + TIE_CHORD_WEIGHT * chord_share
}

fn is_tonic_chord(chord: Chord, key: Key) -> bool {
    use ChordQuality::*;
    chord.root == key.tonic
        && if key.minor { matches!(chord.quality, Minor | Minor7) } else { matches!(chord.quality, Major | Major7 | Dominant7) }
}

/// Inter-onset segments (about one beat each in a loop), or fixed windows without onsets.
fn segments(features: &Features) -> Vec<std::ops::Range<usize>> {
    let n = features.chroma.len();
    let mut bounds: Vec<usize> = if features.onsets.len() >= 2 {
        features.onsets.clone()
    } else {
        let step = ((SEGMENT_SECONDS / features.hop_seconds) as usize).max(1);
        (0..n).step_by(step).collect()
    };
    bounds.push(n);
    bounds.windows(2).filter(|w| w[0] < w[1]).map(|w| w[0]..w[1]).collect()
}

/// Bass chroma as shares of its own total, or all zeros when the bass is too weak to count.
fn bass_share(frames: &[[f32; 12]], chroma: &[f32; 12]) -> [f32; 12] {
    let bass = sum(frames);
    let total: f32 = bass.iter().sum();
    if total <= 0.0 || total < BASS_PRESENCE * chroma.iter().sum::<f32>() {
        return [0.0; 12];
    }
    bass.map(|v| v / total)
}

fn sum(frames: &[[f32; 12]]) -> [f32; 12] {
    let mut out = [0f32; 12];
    for frame in frames {
        out.iter_mut().zip(frame).for_each(|(o, v)| *o += v);
    }
    out
}

fn dot(a: &[f32; 12], b: &[f32; 12]) -> f32 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

fn l2(a: &[f32; 12]) -> f32 {
    dot(a, a).sqrt()
}

fn pearson(a: &[f32; 12], b: &[f32; 12]) -> f32 {
    let (ma, mb) = (a.iter().sum::<f32>() / 12.0, b.iter().sum::<f32>() / 12.0);
    let (da, db) = (a.map(|v| v - ma), b.map(|v| v - mb));
    let denom = l2(&da) * l2(&db);
    if denom > 0.0 { dot(&da, &db) / denom } else { 0.0 }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analyze::features::extract;
    use crate::analyze::synth::{chord, mix, progression, tone};

    fn key_label(chords: &[&[f32]], bass: &[f32]) -> String {
        let signal = progression(chords, bass, 90.0, 2, 1);
        estimate_key(&extract(&signal)).expect("a key").value.label()
    }

    #[test]
    fn chords_of_each_quality() {
        let qualities: [(&[f32], &str); 6] = [
            (&[0.0, 4.0, 7.0], ""),
            (&[0.0, 3.0, 7.0], "m"),
            (&[0.0, 4.0, 7.0, 10.0], "7"),
            (&[0.0, 4.0, 7.0, 11.0], "maj7"),
            (&[0.0, 3.0, 7.0, 10.0], "m7"),
            (&[0.0, 3.0, 6.0], "dim"),
        ];
        for (root, name) in [(60.0, "C"), (63.0, "Eb")] {
            for (intervals, suffix) in qualities {
                let midis: Vec<f32> = intervals.iter().map(|i| root + i).collect();
                let found = estimate_chord(&extract(&chord(&midis, 1.5))).expect("a chord");
                assert_eq!(found.value.label(), format!("{name}{suffix}"), "{midis:?}");
            }
        }
    }

    #[test]
    fn inversion_root_comes_from_bass() {
        let signal = mix(&chord(&[64.0, 67.0, 72.0], 1.5), &tone(48.0, 1.5, 6));
        assert_eq!(estimate_chord(&extract(&signal)).unwrap().value.label(), "C");
    }

    #[test]
    fn key_of_progression() {
        let (am, dm, em): (&[f32], &[f32], &[f32]) = (&[57.0, 60.0, 64.0], &[62.0, 65.0, 69.0], &[64.0, 67.0, 71.0]);
        assert_eq!(key_label(&[am, dm, em, am], &[45.0, 50.0, 52.0, 45.0]), "Am");
        let (eb, ab, bb): (&[f32], &[f32], &[f32]) = (&[63.0, 67.0, 70.0], &[68.0, 72.0, 75.0], &[70.0, 74.0, 77.0]);
        assert_eq!(key_label(&[eb, ab, bb, eb], &[51.0, 56.0, 58.0, 51.0]), "Eb");
    }

    #[test]
    fn relative_tiebreak_uses_bass_and_ends() {
        let (am, f, c, g): (&[f32], &[f32], &[f32], &[f32]) =
            (&[57.0, 60.0, 64.0], &[57.0, 60.0, 65.0], &[60.0, 64.0, 67.0], &[59.0, 62.0, 67.0]);
        assert_eq!(key_label(&[am, f, c, g, am], &[45.0, 41.0, 48.0, 43.0, 45.0]), "Am");
        assert_eq!(key_label(&[c, am, f, g, c], &[48.0, 45.0, 41.0, 43.0, 48.0]), "C");
    }

    #[test]
    fn tie_break_overrides_a_close_profile_call() {
        // The profiles alone prefer C here; bass, ends and chord counts all say Am.
        let (am, c, g): (&[f32], &[f32], &[f32]) = (&[57.0, 60.0, 64.0], &[60.0, 64.0, 67.0], &[59.0, 62.0, 67.0]);
        let signal = progression(&[am, c, g, c, am], &[45.0, 48.0, 43.0, 48.0, 45.0], 90.0, 2, 1);
        let features = extract(&signal);
        let chroma = sum(&features.chroma);
        let (a_minor, c_major) = (Key { tonic: PitchClass(9), minor: true }, Key { tonic: PitchClass(0), minor: false });
        assert!(key_correlation(&chroma, c_major) > key_correlation(&chroma, a_minor));
        assert_eq!(estimate_key(&features).unwrap().value, a_minor);
    }

    #[test]
    fn tie_break_can_pick_the_major_key() {
        // Mirror of the test above: the profiles prefer Am; bass, ends and chord counts say C.
        let (am, c, f): (&[f32], &[f32], &[f32]) = (&[57.0, 60.0, 64.0], &[60.0, 64.0, 67.0], &[57.0, 60.0, 65.0]);
        let signal = progression(&[c, am, f, am, c], &[48.0, 45.0, 41.0, 45.0, 48.0], 90.0, 2, 1);
        let features = extract(&signal);
        let chroma = sum(&features.chroma);
        let (a_minor, c_major) = (Key { tonic: PitchClass(9), minor: true }, Key { tonic: PitchClass(0), minor: false });
        assert!(key_correlation(&chroma, a_minor) > key_correlation(&chroma, c_major));
        assert_eq!(estimate_key(&features).unwrap().value, c_major);
    }

    #[test]
    fn near_tie_with_another_key_is_not_confident() {
        // Built from profiles: C is best, its relative Am is close enough for the tie-break,
        // and Em (not part of that pair) is within a hair of C, so the result is uncertain.
        let major = |t: usize| -> [f32; 12] { std::array::from_fn(|pc| MAJOR_PROFILE[(pc + 12 - t) % 12]) };
        let minor = |t: usize| -> [f32; 12] { std::array::from_fn(|pc| MINOR_PROFILE[(pc + 12 - t) % 12]) };
        let frame: [f32; 12] =
            std::array::from_fn(|pc| major(0)[pc] + 3.0 * minor(9)[pc] + 0.95 * (major(7)[pc] + 3.0 * minor(4)[pc]));
        let n = 100;
        let features = Features {
            hop_seconds: 0.023,
            onset_env: vec![0.0; n],
            onsets: Vec::new(),
            chroma: vec![frame; n],
            bass_chroma: vec![[0.0; 12]; n],
            tuning_cents: 0.0,
            rms: vec![0.1; n],
        };
        let key = |t: u8, minor: bool| Key { tonic: PitchClass(t), minor };
        let corr = |k: Key| key_correlation(&frame, k);
        assert!(corr(key(0, false)) > corr(key(4, true)) && corr(key(0, false)) - corr(key(4, true)) < 0.02);
        assert!(corr(key(0, false)) - corr(key(9, true)) < RELATIVE_MARGIN, "must take the tie-break path");
        let found = estimate_key(&features).unwrap();
        assert!(!found.confident, "{found:?}");
    }

    #[test]
    fn chords_are_polyphonic_and_notes_are_not() {
        assert!(is_polyphonic(&extract(&chord(&[60.0, 64.0, 67.0], 1.5))));
        assert!(!is_polyphonic(&extract(&tone(57.0, 1.5, 8))));
    }
}

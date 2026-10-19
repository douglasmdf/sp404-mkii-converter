//! Sample kind: one-shot or loop, pitched or not, and which pitched kind — from the sound,
//! nudged by words in the folder and file names when the sound alone is not clear.
//!
//! Each yes/no question (loop? pitched?) gets a 0–1 score from the sound. At or above the
//! "strong" bound (or at or below the "weak" one) the sound decides alone and is trusted.
//! In between, the name hints decide; with no hints the score decides against a midpoint.
//! The answer is uncertain when the sound is not strong and the hints did not agree with it.

use super::features::Features;
use super::harmony::is_polyphonic;
use super::pitch::{f0_track, pitch_stability};
use super::signal::Signal;
use super::tempo::periodicity;
use super::{Estimate, NameHints};
use crate::tags::SampleKind;

/// Measurements of the sound used by the classifier.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Sound {
    pub seconds: f32,
    pub onsets: usize,
    pub periodicity: f32,
    pub stability: f32,
    pub tonality: f32,
    pub polyphonic: bool,
    pub held_notes: usize,
}

pub(crate) fn measure(signal: &Signal, features: &Features, periodicity: f32) -> Sound {
    let track = f0_track(signal);
    Sound {
        seconds: signal.source_seconds as f32,
        onsets: features.onsets.len(),
        periodicity,
        stability: pitch_stability(&track),
        tonality: tonality(features),
        polyphonic: is_polyphonic(features),
        held_notes: held_notes(&track),
    }
}

/// Energy-weighted share of each chroma frame held by its three strongest pitch classes:
/// near 1 for notes and chords, much lower for noise.
fn tonality(features: &Features) -> f32 {
    let (mut top, mut total) = (0f32, 0f32);
    for frame in &features.chroma {
        let mut sorted = *frame;
        sorted.sort_by(|a, b| b.total_cmp(a));
        top += sorted[..3].iter().sum::<f32>();
        total += frame.iter().sum::<f32>();
    }
    if total > 0.0 { top / total } else { 0.0 }
}

/// Frames a note must hold (clear f0, same rounded note) to count as a note of a phrase.
const HELD_FRAMES: usize = 4;
const HELD_CLARITY: f32 = 0.8;

/// Number of distinct notes that are each held for at least `HELD_FRAMES` frames in a row.
fn held_notes(track: &[Option<(f32, f32)>]) -> usize {
    let mut notes: Vec<i32> = Vec::new();
    let mut run: Option<(i32, usize)> = None;
    for frame in track {
        let note = frame.filter(|(_, c)| *c >= HELD_CLARITY).map(|(hz, _)| (69.0 + 12.0 * (hz / 440.0).log2()).round() as i32);
        run = match (run, note) {
            (Some((n, len)), Some(m)) if n == m => Some((n, len + 1)),
            (_, Some(m)) => Some((m, 1)),
            (_, None) => None,
        };
        if let Some((n, len)) = run {
            if len == HELD_FRAMES && !notes.contains(&n) {
                notes.push(n);
            }
        }
    }
    notes.len()
}

// Loop question: periodicity of the onsets, faded in between these lengths (seconds)...
const LOOP_MIN_SECONDS: f32 = 1.0;
const LOOP_FULL_SECONDS: f32 = 2.0;
/// ...and only with at least this many onsets.
const LOOP_MIN_ONSETS: usize = 3;
const LOOP_STRONG: f32 = 0.5;
const LOOP_MIDPOINT: f32 = 0.35;
const LOOP_WEAK: f32 = 0.15;
// Pitched question: tonality mapped from these bounds onto 0–1...
const TONALITY_NONE: f32 = 0.5;
const TONALITY_FULL: f32 = 0.8;
/// ...times the f0 stability (or 1 for a clearly tonal polyphonic sound), faded in over
/// this length, since a very short sound has too few frames for a reliable pitch.
const PITCH_FULL_SECONDS: f32 = 0.25;
const PITCH_STRONG: f32 = 0.7;
const PITCH_MIDPOINT: f32 = 0.45;
const PITCH_WEAK: f32 = 0.2;
/// Polyphony only means a chord when the sound is this tonal (noise is "polyphonic" too).
const CHORD_MIN_TONALITY: f32 = 0.7;
/// A phrase has at least this many held notes and onsets.
const PHRASE_MIN_NOTES: usize = 2;
const PHRASE_MIN_ONSETS: usize = 2;

const DRUM_WORDS: &[&str] = &[
    "kick", "kicks", "snare", "hat", "hats", "hihat", "hi-hat", "clap", "perc", "percussion", "rim", "tom", "cymbal",
    "crash", "ride", "shaker", "drum", "drums", "break", "breaks", "groove", "top", "tops", "fx",
];
const ONE_SHOT_WORDS: &[&str] = &["one shot", "one-shot", "oneshot", "one shots", "hit", "hits", "shot", "stab"];
const LOOP_WORDS: &[&str] = &["loop", "loops"];
const CHORD_WORDS: &[&str] = &["chord", "chords"];
const PITCHED_WORDS: &[&str] = &["808", "bass", "keys", "piano", "synth", "pad", "lead", "guitar", "melody", "melodic", "vocal"];
/// Pitched even inside drum folders.
const ALWAYS_PITCHED_WORDS: &[&str] = &["808", "bass"];

pub fn classify(signal: &Signal, features: &Features, hints: &NameHints) -> Estimate<SampleKind> {
    classify_with(signal, features, hints, periodicity(features))
}

/// `classify` with the onset periodicity already computed (it is shared with the tempo).
pub(crate) fn classify_with(signal: &Signal, features: &Features, hints: &NameHints, periodicity: f32) -> Estimate<SampleKind> {
    let sound = measure(signal, features, periodicity);
    let has = |list: &[&str]| list.iter().any(|w| hints.has_word(w));

    let loop_hint = sign(i32::from(has(LOOP_WORDS)) + i32::from(hints.has_bpm_token) - i32::from(has(ONE_SHOT_WORDS)));
    let pitched_hint = if has(ALWAYS_PITCHED_WORDS) {
        1
    } else {
        sign(i32::from(has(PITCHED_WORDS) || has(CHORD_WORDS) || hints.has_key_token) - i32::from(has(DRUM_WORDS)))
    };
    let (is_loop, loop_sure) = decide(loop_score(&sound), (LOOP_WEAK, LOOP_MIDPOINT, LOOP_STRONG), loop_hint);
    let (pitched, pitch_sure) = decide(pitch_score(&sound), (PITCH_WEAK, PITCH_MIDPOINT, PITCH_STRONG), pitched_hint);
    let sure = loop_sure && pitch_sure;

    let (value, confident) = match (is_loop, pitched) {
        (true, true) => (SampleKind::MelodicLoop, sure),
        (true, false) => (SampleKind::DrumLoop, sure),
        (false, false) => (SampleKind::Unpitched, sure),
        (false, true) => {
            if sound.polyphonic && sound.tonality >= CHORD_MIN_TONALITY {
                (SampleKind::Chord, sure)
            } else if sound.held_notes >= PHRASE_MIN_NOTES && sound.onsets >= PHRASE_MIN_ONSETS {
                (SampleKind::Phrase, sure)
            } else if has(CHORD_WORDS) {
                (SampleKind::Chord, false)
            } else {
                (SampleKind::Note, sure)
            }
        }
    };
    Estimate { value, confident }
}

fn loop_score(sound: &Sound) -> f32 {
    if sound.onsets < LOOP_MIN_ONSETS {
        return 0.0;
    }
    sound.periodicity * ramp(sound.seconds, LOOP_MIN_SECONDS, LOOP_FULL_SECONDS)
}

fn pitch_score(sound: &Sound) -> f32 {
    let tonal = ramp(sound.tonality, TONALITY_NONE, TONALITY_FULL);
    let clear = if sound.polyphonic && sound.tonality >= CHORD_MIN_TONALITY { 1.0 } else { sound.stability };
    tonal * clear * ramp(sound.seconds, 0.0, PITCH_FULL_SECONDS)
}

/// `(answer, confident)` for one question; see the module comment.
fn decide(score: f32, (weak, midpoint, strong): (f32, f32, f32), hint: i32) -> (bool, bool) {
    if score >= strong {
        return (true, true);
    }
    if score <= weak {
        return (false, true);
    }
    let leaning = score >= midpoint;
    match hint {
        0 => (leaning, false),
        h => (h > 0, (h > 0) == leaning),
    }
}

/// 0 at or below `lo`, 1 at or above `hi`, linear in between.
fn ramp(v: f32, lo: f32, hi: f32) -> f32 {
    ((v - lo) / (hi - lo)).clamp(0.0, 1.0)
}

fn sign(v: i32) -> i32 {
    v.signum()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analyze::features::extract;
    use crate::analyze::synth::{arpeggio, chord, click_loop, noise_hit, progression, tone};
    use crate::name_tags::parse_name;

    fn kind(signal: &Signal, hints: &NameHints) -> SampleKind {
        classify(signal, &extract(signal), hints).value
    }

    fn hints(dirs: &[&str], stem: &str) -> NameHints {
        let dirs: Vec<String> = dirs.iter().map(|d| d.to_string()).collect();
        NameHints::from_path(&dirs, stem, &parse_name(stem))
    }

    /// A damped sine that dies out in about 60 ms: tonal, but too short for a sure pitch.
    fn short_sine() -> Signal {
        let rate = crate::analyze::ANALYSIS_RATE as f32;
        let n = (0.15 * rate) as usize;
        let samples = (0..n)
            .map(|i| {
                let t = i as f32 / rate;
                0.5 * (std::f32::consts::TAU * 150.0 * t).sin() * (-t / 0.015).exp()
            })
            .collect();
        Signal { samples, rate: rate as u32, source_seconds: n as f64 / rate as f64 }
    }

    #[test]
    fn classify_synthetic_kinds() {
        let none = NameHints::default();
        let (am, dm, em): (&[f32], &[f32], &[f32]) = (&[57.0, 60.0, 64.0], &[62.0, 65.0, 69.0], &[64.0, 67.0, 71.0]);
        assert_eq!(kind(&noise_hit(0.3), &none), SampleKind::Unpitched);
        assert_eq!(kind(&tone(36.0, 1.5, 8), &none), SampleKind::Note);
        assert_eq!(kind(&chord(&[60.0, 63.0, 67.0], 2.0), &none), SampleKind::Chord);
        assert_eq!(kind(&arpeggio(&[57.0, 60.0, 64.0, 67.0], 0.25), &none), SampleKind::Phrase);
        let prog = progression(&[am, dm, em, am], &[45.0, 50.0, 52.0, 45.0], 90.0, 2, 4);
        assert_eq!(kind(&prog, &none), SampleKind::MelodicLoop);
        assert_eq!(kind(&click_loop(90.0, 16), &none), SampleKind::DrumLoop);
    }

    #[test]
    fn hints_decide_ambiguous_cases() {
        let short = short_sine();
        let sound = classify(&short, &extract(&short), &NameHints::default());
        assert!(!sound.confident, "the short sine must be ambiguous: {sound:?}");
        assert_eq!(kind(&short, &hints(&[], "kick")), SampleKind::Unpitched);
        assert_eq!(kind(&short, &hints(&[], "808")), SampleKind::Note);
        let mut loop_hints = hints(&["one shots"], "snare");
        assert!(loop_hints.words.contains(&"oneshot".to_string()), "{loop_hints:?}");
        loop_hints.has_bpm_token = true;
        assert_eq!(kind(&click_loop(90.0, 16), &loop_hints), SampleKind::DrumLoop);
    }

    #[test]
    fn bass_in_drum_folder_is_pitched() {
        let short = short_sine();
        assert_eq!(kind(&short, &hints(&["Drums"], "bass hit")), SampleKind::Note);
    }

    #[test]
    fn disagreeing_hints_on_weak_sound_are_not_confident() {
        let short = short_sine();
        let found = classify(&short, &extract(&short), &hints(&[], "808"));
        let leaning = classify(&short, &extract(&short), &NameHints::default());
        assert_ne!(found.value, leaning.value);
        assert!(!found.confident);
        // Strong sound wins over hints and stays confident.
        let clicks = click_loop(90.0, 16);
        let strong = classify(&clicks, &extract(&clicks), &hints(&["One Shots"], "piano"));
        assert_eq!(strong.value, SampleKind::DrumLoop);
        assert!(strong.confident);
    }
}

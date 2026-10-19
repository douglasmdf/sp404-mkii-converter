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
    pub partials: f32,
    pub glide: f32,
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
        partials: features.sustained_partials,
        glide: glide(&track, features.tuning_cents),
    }
}

/// A clear pitch must cover this many frames, and more than `GLIDE_MIN_RANGE` semitones
/// (10th to 90th percentile), before it can count as a glide...
const GLIDE_MIN_FRAMES: usize = 10;
const GLIDE_MIN_RANGE: f32 = 1.0;
/// ...and the mean distance of the pitch from the nearest tempered note (cents) is mapped
/// from these bounds onto 0–1. Notes, chords and phrases settle on notes (3–16 cents in the
/// test pack); sirens and risers slide through them (20–30 cents).
const GLIDE_NONE_CENTS: f32 = 15.0;
const GLIDE_FULL_CENTS: f32 = 22.0;

/// 0–1: how much the pitch slides instead of settling on notes.
fn glide(track: &[Option<(f32, f32)>], tuning_cents: f32) -> f32 {
    let mut midis: Vec<f32> = track
        .iter()
        .flatten()
        .filter(|(_, c)| *c >= HELD_CLARITY)
        .map(|(hz, _)| 69.0 + 12.0 * (hz / 440.0).log2() - tuning_cents / 100.0)
        .collect();
    if midis.len() < GLIDE_MIN_FRAMES {
        return 0.0;
    }
    let cents = midis.iter().map(|m| (m - m.round()).abs() * 100.0).sum::<f32>() / midis.len() as f32;
    midis.sort_by(f32::total_cmp);
    let range = midis[midis.len() * 9 / 10] - midis[midis.len() / 10];
    if range <= GLIDE_MIN_RANGE {
        return 0.0;
    }
    ramp(cents, GLIDE_NONE_CENTS, GLIDE_FULL_CENTS)
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
/// Notes are rounded after removing the sound's own tuning (circular mean of the clear f0
/// offsets), so a single note tuned near a quarter tone (`Mallet Roll` sits ~45 cents off)
/// does not split into two.
fn held_notes(track: &[Option<(f32, f32)>]) -> usize {
    let midi = |hz: f32| 69.0 + 12.0 * (hz / 440.0).log2();
    let (mut re, mut im) = (0f32, 0f32);
    for (hz, _) in track.iter().flatten().filter(|(_, c)| *c >= HELD_CLARITY) {
        let angle = std::f32::consts::TAU * midi(*hz);
        re += angle.cos();
        im += angle.sin();
    }
    let offset = if re == 0.0 && im == 0.0 { 0.0 } else { im.atan2(re) / std::f32::consts::TAU };
    let mut notes: Vec<i32> = Vec::new();
    let mut run: Option<(i32, usize)> = None;
    for frame in track {
        let note = frame.filter(|(_, c)| *c >= HELD_CLARITY).map(|(hz, _)| (midi(hz) - offset).round() as i32);
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

// Loop question (each part 0–1, the larger wins): a steady pulse at the tempo the length
// implies (tempo::Rhythm::periodicity mapped from these bounds), faded in between these
// lengths (seconds)...
const PULSE_NONE: f32 = 0.06;
const PULSE_FULL: f32 = 0.16;
const PULSE_MIN_SECONDS: f32 = 2.0;
const PULSE_FULL_SECONDS: f32 = 4.0;
/// ...or simply a long sound with many onsets: one-shots in the test pack last at most 17 s,
/// melodic loops 23–91 s, and half of those have too soft a pulse for the first part.
const LONG_MIN_SECONDS: f32 = 15.0;
const LONG_FULL_SECONDS: f32 = 25.0;
/// Both parts need at least this many onsets.
const LOOP_MIN_ONSETS: usize = 3;
const LOOP_STRONG: f32 = 0.5;
const LOOP_MIDPOINT: f32 = 0.35;
const LOOP_WEAK: f32 = 0.15;
// Pitched question: the larger of the sustained-partial share mapped from these bounds (drum
// loops and hats stay under ~0.22, melodic loops and pitched one-shots mostly above 0.25)...
const PARTIALS_NONE: f32 = 0.12;
const PARTIALS_FULL: f32 = 0.32;
/// ...and the f0 measure for short notes: tonality mapped from these bounds onto 0–1...
const TONALITY_NONE: f32 = 0.5;
const TONALITY_FULL: f32 = 0.8;
/// ...times the f0 stability (or 1 for a clearly tonal polyphonic sound), faded in over
/// this length, since a very short sound has too few frames for a reliable pitch. Both are
/// scaled down by the glide (a siren has a clear pitch but no note).
const PITCH_FULL_SECONDS: f32 = 0.25;
const PITCH_STRONG: f32 = 0.7;
const PITCH_MIDPOINT: f32 = 0.45;
const PITCH_WEAK: f32 = 0.2;
/// Polyphony counts as a clear pitch for the f0 measure only when the sound is this tonal
/// (noise is "polyphonic" too).
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
    classify_with(signal, features, hints, periodicity(features, signal.file_seconds))
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
    let (is_loop, loop_sure) = if cut_to_name_tempo(signal.file_seconds, hints) {
        (true, true)
    } else {
        decide(loop_score(&sound), (LOOP_WEAK, LOOP_MIDPOINT, LOOP_STRONG), loop_hint)
    };
    let (pitched, pitch_sure) = decide(pitch_score(&sound), (PITCH_WEAK, PITCH_MIDPOINT, PITCH_STRONG), pitched_hint);
    let sure = loop_sure && pitch_sure;

    let (value, confident) = match (is_loop, pitched) {
        (true, true) => (SampleKind::MelodicLoop, sure),
        (true, false) => (SampleKind::DrumLoop, sure),
        (false, false) => (SampleKind::Unpitched, sure),
        (false, true) => {
            // The sound already passed the pitched question, so polyphony alone means a chord
            // (the tonality bound would reject chords of four or more notes, whose energy is
            // spread over more than three pitch classes: `Horn Stab wBass`).
            if sound.polyphonic {
                (SampleKind::Chord, sure)
            } else if sound.held_notes >= PHRASE_MIN_NOTES && sound.onsets >= PHRASE_MIN_ONSETS {
                // Just two held notes may be one note read twice (octave or tuning slips).
                (SampleKind::Phrase, sure && sound.held_notes > PHRASE_MIN_NOTES)
            } else if has(CHORD_WORDS) {
                (SampleKind::Chord, false)
            } else {
                (SampleKind::Note, sure)
            }
        }
    };
    Estimate { value, confident }
}

/// A file at least `GRID_MIN_BEATS` long whose length is within `GRID_TOLERANCE_BEATS` of a
/// whole number of beats at the BPM its name states was cut to that tempo: a loop, whatever
/// the onsets say (in the test pack, two-beat tambourine loops with a single shake).
const GRID_MIN_BEATS: f64 = 2.0;
const GRID_TOLERANCE_BEATS: f64 = 0.05;

fn cut_to_name_tempo(seconds: f64, hints: &NameHints) -> bool {
    hints.name_bpm().is_some_and(|bpm| {
        let beats = seconds * bpm as f64 / 60.0;
        beats.round() >= GRID_MIN_BEATS && (beats - beats.round()).abs() <= GRID_TOLERANCE_BEATS
    })
}

fn loop_score(sound: &Sound) -> f32 {
    if sound.onsets < LOOP_MIN_ONSETS {
        return 0.0;
    }
    let pulse = ramp(sound.periodicity, PULSE_NONE, PULSE_FULL) * ramp(sound.seconds, PULSE_MIN_SECONDS, PULSE_FULL_SECONDS);
    pulse.max(ramp(sound.seconds, LONG_MIN_SECONDS, LONG_FULL_SECONDS))
}

fn pitch_score(sound: &Sound) -> f32 {
    let tonal = ramp(sound.tonality, TONALITY_NONE, TONALITY_FULL);
    let clear = if sound.polyphonic && sound.tonality >= CHORD_MIN_TONALITY { 1.0 } else { sound.stability };
    let f0 = tonal * clear * ramp(sound.seconds, 0.0, PITCH_FULL_SECONDS);
    f0.max(ramp(sound.partials, PARTIALS_NONE, PARTIALS_FULL)) * (1.0 - sound.glide)
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
        let seconds = n as f64 / rate as f64;
        Signal { samples, rate: rate as u32, source_seconds: seconds, file_seconds: seconds }
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
    fn length_on_the_named_tempo_makes_a_loop() {
        // One hit in a two-beat file: a loop only because the name's BPM fits its length.
        let mut hit = noise_hit(0.3);
        hit.file_seconds = 2.0 * 60.0 / 85.0;
        assert_eq!(kind(&hit, &hints(&["One Shots"], "Tambo (85bpm)")), SampleKind::DrumLoop);
        assert_eq!(kind(&hit, &hints(&["One Shots"], "Tambo (97bpm)")), SampleKind::Unpitched);
        assert_eq!(kind(&hit, &hints(&["One Shots"], "Tambo")), SampleKind::Unpitched);
    }

    /// A siren: a clear pitch that sweeps up and down two octaves without settling on notes.
    fn siren() -> Signal {
        let rate = crate::analyze::ANALYSIS_RATE as f32;
        let n = (4.0 * rate) as usize;
        let mut phase = 0f32;
        let samples = (0..n)
            .map(|i| {
                let t = i as f32 / rate;
                let midi = 60.0 + 12.0 * (1.0 - (std::f32::consts::PI * t / 2.0).cos());
                phase = (phase + std::f32::consts::TAU * 440.0 * 2f32.powf((midi - 69.0) / 12.0) / rate) % std::f32::consts::TAU;
                0.4 * (phase.sin() + 0.3 * (2.0 * phase).sin())
            })
            .collect();
        Signal { samples, rate: rate as u32, source_seconds: 4.0, file_seconds: 4.0 }
    }

    #[test]
    fn a_siren_has_no_note() {
        let s = siren();
        let found = classify(&s, &extract(&s), &NameHints::default());
        assert_eq!(found.value, SampleKind::Unpitched, "{found:?}");
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

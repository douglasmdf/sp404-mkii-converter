//! Deterministic synthetic test signals at `ANALYSIS_RATE`.
#![allow(dead_code)] // shared by the analyzer tests; not every test uses every signal

use std::f32::consts::TAU;

use super::signal::Signal;
use super::ANALYSIS_RATE;

const RATE: f32 = ANALYSIS_RATE as f32;
const ATTACK_SECONDS: f32 = 0.005;
const RELEASE_SECONDS: f32 = 0.01;
const PEAK: f32 = 0.5;

pub(crate) fn midi_hz(midi: f32) -> f32 {
    440.0 * 2f32.powf((midi - 69.0) / 12.0)
}

/// A plucked harmonic tone: harmonics at 1/k amplitude, fast attack, slow decay.
pub(crate) fn tone(midi: f32, seconds: f32, harmonics: usize) -> Signal {
    signal(normalized(tone_samples(midi, seconds, harmonics, 1.0)))
}

/// An 808-style bass: pitch falls from `from_midi` to `to_midi` in the first ~0.2 s, then holds.
pub(crate) fn glide_808(from_midi: f32, to_midi: f32, seconds: f32) -> Signal {
    let n = frames(seconds);
    let mut phase = 0f32;
    let samples = (0..n)
        .map(|i| {
            let t = i as f32 / RATE;
            let midi = to_midi + (from_midi - to_midi) * (-t / 0.05).exp();
            phase = (phase + TAU * midi_hz(midi) / RATE) % TAU;
            let wave = phase.sin() + 0.3 * (2.0 * phase).sin() + 0.15 * (3.0 * phase).sin();
            wave * envelope(i, n, 1.5)
        })
        .collect();
    signal(normalized(samples))
}

pub(crate) fn chord(midis: &[f32], seconds: f32) -> Signal {
    let mut out = vec![0.0; frames(seconds)];
    for &m in midis {
        add_at(&mut out, 0, &tone_samples(m, seconds, 4, 1.0), 1.0);
    }
    signal(normalized(out))
}

/// A chord whose notes enter `step` seconds apart and then all ring to the end (a rolled
/// chord, or an arpeggio played through a long delay).
pub(crate) fn rolled_chord(midis: &[f32], step: f32, seconds: f32) -> Signal {
    let mut out = vec![0.0; frames(seconds)];
    for (i, &m) in midis.iter().enumerate() {
        let start = frames(i as f32 * step);
        add_at(&mut out, start, &tone_samples(m, seconds - i as f32 * step, 4, 1.0), 1.0);
    }
    signal(normalized(out))
}

/// A long pad with no pulse: notes from an A-minor scale enter at irregular gaps (1.5–6 s,
/// fixed seed) and ring for several seconds.
pub(crate) fn drifting_pad(seconds: f32) -> Signal {
    let scale = [45.0, 57.0, 60.0, 62.0, 64.0, 67.0, 69.0, 72.0];
    let mut rng = Lcg(2024);
    let mut out = vec![0.0; frames(seconds)];
    let mut t = 0.0;
    let mut i = 0;
    while t < seconds - 1.0 {
        let note = scale[i % scale.len()];
        let len = (seconds - t).min(8.0);
        add_at(&mut out, frames(t), &tone_samples(note, len, 4, 0.4), 0.5);
        t += 1.5 + 2.25 * (rng.next() + 1.0);
        i += 3;
    }
    signal(normalized(out))
}

/// The notes one after the other.
pub(crate) fn arpeggio(midis: &[f32], note_seconds: f32) -> Signal {
    let samples = midis.iter().flat_map(|&m| tone_samples(m, note_seconds, 4, 3.0)).collect();
    signal(normalized(samples))
}

/// One click per beat, cut exactly at `beats` beats.
pub(crate) fn click_loop(bpm: f32, beats: usize) -> Signal {
    let beat = 60.0 / bpm;
    let mut out = vec![0.0; frames(beat * beats as f32)];
    let click = click_samples();
    for b in 0..beats {
        add_at(&mut out, (b as f32 * beat * RATE).round() as usize, &click, 1.0);
    }
    signal(out)
}

/// Chords re-struck on every beat over a bass note held for the whole chord.
pub(crate) fn progression(chords: &[&[f32]], bass: &[f32], bpm: f32, beats_per_chord: usize, repeats: usize) -> Signal {
    let beat = 60.0 / bpm;
    let chord_len = beat * beats_per_chord as f32;
    let mut out = vec![0.0; frames(chord_len * (chords.len() * repeats) as f32)];
    for r in 0..repeats {
        for (c, notes) in chords.iter().enumerate() {
            let chord_start = (r * chords.len() + c) as f32 * chord_len;
            for b in 0..beats_per_chord {
                let start = ((chord_start + b as f32 * beat) * RATE).round() as usize;
                for &m in notes.iter() {
                    add_at(&mut out, start, &tone_samples(m, beat, 4, 2.0), 0.3);
                }
            }
            if let Some(&m) = bass.get(c) {
                add_at(&mut out, (chord_start * RATE).round() as usize, &tone_samples(m, chord_len, 3, 1.0), 0.6);
            }
        }
    }
    signal(normalized(out))
}

/// A decaying burst of white noise (no pitch).
pub(crate) fn noise_hit(seconds: f32) -> Signal {
    let n = frames(seconds);
    let mut rng = Lcg(12_345);
    let samples = (0..n).map(|i| rng.next() * (-(i as f32 / RATE) / (seconds / 5.0)).exp()).collect();
    signal(normalized(samples))
}

pub(crate) fn mix(a: &Signal, b: &Signal) -> Signal {
    let mut out = a.samples.clone();
    if out.len() < b.samples.len() {
        out.resize(b.samples.len(), 0.0);
    }
    add_at(&mut out, 0, &b.samples, 1.0);
    signal(out)
}

/// `signal` followed by `seconds` of silence.
pub(crate) fn with_tail(signal_in: &Signal, seconds: f32) -> Signal {
    let mut out = signal_in.samples.clone();
    out.resize(out.len() + frames(seconds), 0.0);
    signal(out)
}

fn tone_samples(midi: f32, seconds: f32, harmonics: usize, decay: f32) -> Vec<f32> {
    let n = frames(seconds);
    let f0 = midi_hz(midi);
    let partials: Vec<usize> = (1..=harmonics.max(1)).filter(|&k| k as f32 * f0 < 0.45 * RATE).collect();
    (0..n)
        .map(|i| {
            let t = i as f32 / RATE;
            let wave: f32 = partials.iter().map(|&k| (TAU * k as f32 * f0 * t).sin() / k as f32).sum();
            wave * envelope(i, n, decay)
        })
        .collect()
}

fn click_samples() -> Vec<f32> {
    let mut rng = Lcg(777);
    (0..frames(0.03)).map(|i| 0.8 * rng.next() * (-(i as f32 / RATE) / 0.004).exp()).collect()
}

/// Linear attack, exponential decay, short linear release.
fn envelope(i: usize, n: usize, decay: f32) -> f32 {
    let t = i as f32 / RATE;
    let left = (n - i) as f32 / RATE;
    let attack = (t / ATTACK_SECONDS).min(1.0);
    let release = (left / RELEASE_SECONDS).min(1.0);
    attack * release * (-t * decay).exp()
}

fn add_at(out: &mut Vec<f32>, start: usize, samples: &[f32], gain: f32) {
    if out.len() < start + samples.len() {
        out.resize(start + samples.len(), 0.0);
    }
    for (o, s) in out[start..].iter_mut().zip(samples) {
        *o += gain * s;
    }
}

fn normalized(mut samples: Vec<f32>) -> Vec<f32> {
    let peak = samples.iter().fold(0f32, |m, s| m.max(s.abs()));
    if peak > 0.0 {
        samples.iter_mut().for_each(|s| *s *= PEAK / peak);
    }
    samples
}

fn frames(seconds: f32) -> usize {
    (seconds * RATE).round() as usize
}

fn signal(samples: Vec<f32>) -> Signal {
    let source_seconds = samples.len() as f64 / ANALYSIS_RATE as f64;
    Signal { samples, rate: ANALYSIS_RATE, source_seconds, file_seconds: source_seconds, truncated: false }
}

/// Fixed-seed LCG, uniform in [-1, 1).
struct Lcg(u32);

impl Lcg {
    fn next(&mut self) -> f32 {
        self.0 = self.0.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        (self.0 >> 8) as f32 / (1u32 << 23) as f32 - 1.0
    }
}

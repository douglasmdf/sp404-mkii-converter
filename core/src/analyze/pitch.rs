//! Fundamental frequency (YIN) and the note of a pitched one-shot.

use realfft::num_complex::Complex;
use realfft::RealFftPlanner;

use super::features::{Features, HOP};
use super::signal::Signal;
use super::Estimate;
use crate::tags::PitchClass;

const F0_MIN_HZ: f32 = 30.0;
const F0_MAX_HZ: f32 = 2000.0;
/// YIN integration window (~46 ms); with the longest lag it spans ~80 ms per frame.
const YIN_WINDOW: usize = 1024;
/// First dip of the normalized difference below this is taken as the period.
const YIN_THRESHOLD: f32 = 0.15;
/// Frames quieter than this (relative to the signal's peak sample) have no f0.
const SILENT_DB: f32 = -50.0;
/// A frame's f0 counts as clear at this clarity (1 − normalized difference) or more.
const CLEAR_CLARITY: f32 = 0.8;
/// Below this share of clear frames among the voiced frames of the sustained part, the sound
/// has no note.
const MIN_CLEAR_SHARE: f32 = 0.3;
/// The halves of the sustained part differ by more than this (semitones) → treat as a glide.
const GLIDE_SEMITONES: f32 = 0.5;
/// Interquartile range of the f0 (semitones) under which the note is confident.
const CONFIDENT_SPREAD: f32 = 0.5;
/// Consecutive clear frames closer than this (semitones) count as stable.
const STABLE_SEMITONES: f32 = 0.3;

/// (Hz, clarity 0–1) per analysis frame, aligned with `Features` frames (frame `i` is
/// centred on sample `i × HOP`). `None` where the frame is silent or runs past either end.
pub fn f0_track(signal: &Signal) -> Vec<Option<(f32, f32)>> {
    let x = &signal.samples;
    let rate = signal.rate as f32;
    let n_frames = if x.is_empty() { 0 } else { x.len() / HOP + 1 };
    let tau_max = (rate / F0_MIN_HZ).ceil() as usize;
    let tau_min = (rate / F0_MAX_HZ).floor() as usize;
    let span = YIN_WINDOW + tau_max;
    let peak = x.iter().fold(0f32, |m, s| m.max(s.abs()));
    let silent = peak * 10f32.powf(SILENT_DB / 20.0);
    let mut yin = Yin::new(span + YIN_WINDOW, tau_max);
    (0..n_frames)
        .map(|i| {
            let start = (i * HOP).checked_sub(span / 2)?;
            let segment = x.get(start..start + span)?;
            let window = &segment[..YIN_WINDOW];
            let rms = (window.iter().map(|s| s * s).sum::<f32>() / YIN_WINDOW as f32).sqrt();
            if peak <= 0.0 || rms < silent {
                return None;
            }
            let (tau, clarity) = yin.period(segment, tau_min)?;
            Some((rate / tau, clarity))
        })
        .collect()
}

/// Note of a pitched one-shot: median f0 of the clear frames after the attack (of the second
/// half only, when the pitch glides), rounded to the nearest note after removing the tuning.
/// Sounds shorter than about 80 ms (one YIN frame span) have no f0 frames and return `None`.
pub fn one_shot_note(signal: &Signal, features: &Features) -> Option<Estimate<PitchClass>> {
    let track = f0_track(signal);
    let range = features.sustained_range();
    let range = range.start.min(track.len())..range.end.min(track.len());
    if range.is_empty() {
        return None;
    }
    let midis: Vec<f32> = track[range.clone()]
        .iter()
        .flatten()
        .filter(|(_, clarity)| *clarity >= CLEAR_CLARITY)
        .map(|&(hz, _)| hz_to_midi(hz) - features.tuning_cents / 100.0)
        .collect();
    // Frames too close to either end for a YIN window have no f0 and do not count against.
    let voiced = track[range.clone()].iter().flatten().count();
    if midis.is_empty() || (midis.len() as f32) < MIN_CLEAR_SHARE * voiced as f32 {
        return None;
    }
    let (first, second) = midis.split_at(midis.len() / 2);
    let used = if !first.is_empty() && (median(first) - median(second)).abs() > GLIDE_SEMITONES { second } else { &midis[..] };
    let note = median(used).round() as i32;
    let spread = quantile(used, 0.75) - quantile(used, 0.25);
    Some(Estimate { value: PitchClass(note.rem_euclid(12) as u8), confident: spread < CONFIDENT_SPREAD })
}

/// Share of the voiced frames whose f0 is clear and within `STABLE_SEMITONES` of the
/// previous frame's clear f0. Near 1 for a held note, near 0 for noise.
pub fn pitch_stability(track: &[Option<(f32, f32)>]) -> f32 {
    let clear = |f: &Option<(f32, f32)>| f.filter(|(_, c)| *c >= CLEAR_CLARITY).map(|(hz, _)| hz_to_midi(hz));
    let voiced = track.iter().filter(|f| f.is_some()).count();
    if voiced == 0 {
        return 0.0;
    }
    let stable = track
        .windows(2)
        .filter(|w| matches!((clear(&w[0]), clear(&w[1])), (Some(a), Some(b)) if (a - b).abs() < STABLE_SEMITONES))
        .count();
    stable as f32 / voiced as f32
}

fn hz_to_midi(hz: f32) -> f32 {
    69.0 + 12.0 * (hz / 440.0).log2()
}

fn median(values: &[f32]) -> f32 {
    quantile(values, 0.5)
}

fn quantile(values: &[f32], q: f32) -> f32 {
    let mut sorted = values.to_vec();
    sorted.sort_by(f32::total_cmp);
    let pos = q * (sorted.len() - 1) as f32;
    let (lo, hi) = (pos.floor() as usize, pos.ceil() as usize);
    sorted[lo] + (sorted[hi] - sorted[lo]) * (pos - lo as f32)
}

/// YIN with the difference function computed through one FFT cross-correlation per frame.
struct Yin {
    forward: std::sync::Arc<dyn realfft::RealToComplex<f32>>,
    inverse: std::sync::Arc<dyn realfft::ComplexToReal<f32>>,
    size: usize,
    tau_max: usize,
}

impl Yin {
    fn new(min_size: usize, tau_max: usize) -> Yin {
        let size = min_size.next_power_of_two();
        let mut planner = RealFftPlanner::<f32>::new();
        Yin { forward: planner.plan_fft_forward(size), inverse: planner.plan_fft_inverse(size), size, tau_max }
    }

    /// Period in samples (sub-sample, parabolic) and clarity, or `None` with no usable dip.
    fn period(&mut self, segment: &[f32], tau_min: usize) -> Option<(f32, f32)> {
        let w = YIN_WINDOW;
        let cross = self.cross_correlation(segment);
        let mut prefix = vec![0f32; segment.len() + 1];
        for (i, s) in segment.iter().enumerate() {
            prefix[i + 1] = prefix[i] + s * s;
        }
        let e1 = prefix[w];
        // Cumulative-mean-normalized difference d'(τ).
        let mut cmnd = vec![1f32; self.tau_max + 1];
        let mut running = 0f32;
        for tau in 1..=self.tau_max {
            let d = (e1 + prefix[tau + w] - prefix[tau] - 2.0 * cross[tau]).max(0.0);
            running += d;
            cmnd[tau] = if running > 0.0 { d * tau as f32 / running } else { 1.0 };
        }
        let lo = tau_min.max(1);
        let mut tau = match (lo..=self.tau_max).find(|&t| cmnd[t] < YIN_THRESHOLD) {
            Some(t) => t,
            None => (lo..=self.tau_max).min_by(|&a, &b| cmnd[a].total_cmp(&cmnd[b]))?,
        };
        while tau < self.tau_max && cmnd[tau + 1] < cmnd[tau] {
            tau += 1;
        }
        let clarity = (1.0 - cmnd[tau]).clamp(0.0, 1.0);
        let refined = if tau > lo && tau < self.tau_max {
            let (a, b, c) = (cmnd[tau - 1], cmnd[tau], cmnd[tau + 1]);
            let denom = a - 2.0 * b + c;
            if denom.abs() > 1e-12 { tau as f32 + (0.5 * (a - c) / denom).clamp(-0.5, 0.5) } else { tau as f32 }
        } else {
            tau as f32
        };
        Some((refined, clarity))
    }

    /// c(τ) = Σ_{j < YIN_WINDOW} x[j]·x[j + τ] for τ = 0..=tau_max.
    fn cross_correlation(&mut self, segment: &[f32]) -> Vec<f32> {
        let mut full = self.forward.make_input_vec();
        full[..segment.len()].copy_from_slice(segment);
        let mut head = self.forward.make_input_vec();
        head[..YIN_WINDOW].copy_from_slice(&segment[..YIN_WINDOW]);
        let mut full_spec = self.forward.make_output_vec();
        let mut head_spec = self.forward.make_output_vec();
        // Buffer lengths come from the plans, so these cannot fail.
        self.forward.process(&mut full, &mut full_spec).expect("FFT buffers match the plan");
        self.forward.process(&mut head, &mut head_spec).expect("FFT buffers match the plan");
        let mut product: Vec<Complex<f32>> = full_spec.iter().zip(&head_spec).map(|(a, b)| a * b.conj()).collect();
        // The inverse needs purely real DC and Nyquist bins; they are, up to rounding.
        product[0].im = 0.0;
        if let Some(last) = product.last_mut() {
            last.im = 0.0;
        }
        let mut out = self.inverse.make_output_vec();
        self.inverse.process(&mut product, &mut out).expect("FFT buffers match the plan");
        let scale = 1.0 / self.size as f32;
        out.iter().take(self.tau_max + 1).map(|v| v * scale).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analyze::features::extract;
    use crate::analyze::synth::{glide_808, noise_hit, tone};

    fn note_of(signal: &Signal) -> Option<Estimate<PitchClass>> {
        one_shot_note(signal, &extract(signal))
    }

    #[test]
    fn notes_of_harmonic_tones() {
        for midi in [28u8, 36, 45, 57, 64, 72, 84] {
            let note = note_of(&tone(midi as f32, 1.5, 8)).unwrap_or_else(|| panic!("no note for {midi}"));
            assert_eq!(note.value, PitchClass(midi % 12), "midi {midi}");
            assert!(note.confident, "midi {midi}");
        }
    }

    #[test]
    fn detuned_tone_rounds_to_nearest() {
        assert_eq!(note_of(&tone(57.3, 1.5, 8)).unwrap().value, PitchClass(9));
        assert_eq!(note_of(&tone(56.7, 1.5, 8)).unwrap().value, PitchClass(9));
    }

    #[test]
    fn glide_808_uses_arrival_note() {
        assert_eq!(note_of(&glide_808(43.0, 36.0, 1.2)).unwrap().value, PitchClass(0));
    }

    #[test]
    fn lowest_range_notes() {
        assert_eq!(note_of(&tone(24.0, 1.5, 8)).unwrap().value, PitchClass(0));
        assert_eq!(note_of(&glide_808(31.0, 24.0, 1.2)).unwrap().value, PitchClass(0));
    }

    #[test]
    fn noise_has_no_note() {
        assert_eq!(note_of(&noise_hit(0.3)), None);
    }

    #[test]
    fn tones_are_more_stable_than_noise() {
        let tone_stability = pitch_stability(&f0_track(&tone(45.0, 1.5, 8)));
        let noise_stability = pitch_stability(&f0_track(&noise_hit(0.3)));
        assert!(tone_stability > 0.8 && noise_stability < 0.3, "{tone_stability} {noise_stability}");
    }
}

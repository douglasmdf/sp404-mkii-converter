//! Frame-level features: onset envelope, onsets, chroma, bass chroma, tuning and loudness.

use std::f32::consts::TAU;

use realfft::RealFftPlanner;

use super::signal::Signal;

/// Frame step shared by every per-frame feature (~23 ms at 22 050 Hz).
pub const HOP: usize = 512;
/// FFT size for the onset envelope (short, for timing).
const ONSET_WINDOW: usize = 1024;
/// FFT size for chroma (long, for frequency resolution in the bass).
const CHROMA_WINDOW: usize = 4096;
const CHROMA_MIN_HZ: f32 = 55.0;
const CHROMA_MAX_HZ: f32 = 4000.0;
const BASS_MIN_HZ: f32 = 30.0;
const BASS_MAX_HZ: f32 = 250.0;
/// Spectral peaks are also collected up to this frequency, for the sustained-partial share.
const PEAKS_MAX_HZ: f32 = 10_000.0;
/// Sustained partials: peaks in this band (where melodic content lives and drums carry little
/// energy)...
const PARTIAL_MIN_HZ: f32 = 250.0;
const PARTIAL_MAX_HZ: f32 = 2500.0;
/// ...that are present (within this many FFT bins) in each of this many frames before and
/// after (about ±93 ms), so a drum's ringing decay rarely counts but a held note does.
const PARTIAL_HOLD_FRAMES: usize = 4;
const PARTIAL_TOLERANCE_BINS: f32 = 1.5;
/// A spectral peak must be the maximum over this many bins on each side (drops Hann sidelobes).
const PEAK_NEIGHBORS: usize = 2;
/// Peaks quieter than this, relative to the loudest bin of the frame, are ignored.
const PEAK_FLOOR_DB: f32 = -60.0;
/// Absolute floor for peaks (normalized magnitude), so silent frames give no chroma.
const PEAK_ABS_FLOOR: f32 = 1e-5;
/// Log compression of magnitudes before spectral flux: ln(1 + γ·|X|).
const FLUX_COMPRESSION: f32 = 100.0;
/// An onset is the envelope maximum within this many frames on each side...
const ONSET_PEAK_FRAMES: usize = 3;
/// ...is at least this fraction of the envelope maximum...
const ONSET_MIN: f32 = 0.1;
/// ...and exceeds the local mean (over this many frames each side) by `ONSET_DELTA` × maximum.
const ONSET_MEAN_FRAMES: usize = 8;
const ONSET_DELTA: f32 = 0.05;
/// The sustained part starts this many frames after the loudest frame (skips the attack)...
const ATTACK_FRAMES: usize = 3;
/// ...and ends when the level falls this far below the peak.
const SUSTAIN_FLOOR_DB: f32 = -40.0;

#[derive(Debug, Clone, PartialEq)]
pub struct Features {
    pub hop_seconds: f32,
    pub onset_env: Vec<f32>,
    pub onsets: Vec<usize>,
    pub chroma: Vec<[f32; 12]>,
    pub bass_chroma: Vec<[f32; 12]>,
    pub tuning_cents: f32,
    pub rms: Vec<f32>,
    /// Share of all spectral-peak magnitude (30 Hz–10 kHz) held by sustained partials in the
    /// melodic band: near 0 for drums and noise, high for notes, chords and music.
    pub sustained_partials: f32,
}

impl Features {
    /// Frames after the attack while the sound is still clearly audible. Falls back to every
    /// frame when the sound is too short to have a sustained part.
    pub fn sustained_range(&self) -> std::ops::Range<usize> {
        let n = self.rms.len();
        let (peak_at, peak) = self.rms.iter().enumerate().fold((0, 0f32), |b, (i, &v)| if v > b.1 { (i, v) } else { b });
        let floor = peak * 10f32.powf(SUSTAIN_FLOOR_DB / 20.0);
        let end = self.rms.iter().rposition(|&v| v > floor).map_or(0, |i| i + 1);
        let start = peak_at + ATTACK_FRAMES;
        if start < end {
            start..end
        } else {
            0..n
        }
    }
}

pub fn extract(signal: &Signal) -> Features {
    let hop_seconds = HOP as f32 / signal.rate as f32;
    let x = &signal.samples;
    let n_frames = if x.is_empty() { 0 } else { x.len() / HOP + 1 };
    let mut planner = RealFftPlanner::<f32>::new();

    // Spectral peaks per frame; chroma needs the global tuning before peaks become pitch classes.
    let mut long = Spectrum::new(&mut planner, CHROMA_WINDOW);
    let rate = signal.rate as f32;
    let peaks: Vec<Vec<(f32, f32)>> = (0..n_frames)
        .map(|i| spectral_peaks(long.magnitudes(x, i * HOP), rate, CHROMA_WINDOW, BASS_MIN_HZ, PEAKS_MAX_HZ))
        .collect();
    let sustained_partials = sustained_partial_share(&peaks, rate / CHROMA_WINDOW as f32);
    let tuning_cents = estimate_tuning(&peaks);
    let chroma = peaks.iter().map(|p| fold_to_chroma(p, tuning_cents, CHROMA_MIN_HZ, CHROMA_MAX_HZ)).collect();
    let bass_chroma = peaks.iter().map(|p| fold_to_chroma(p, tuning_cents, BASS_MIN_HZ, BASS_MAX_HZ)).collect();

    let mut short = Spectrum::new(&mut planner, ONSET_WINDOW);
    let mut previous = vec![0f32; ONSET_WINDOW / 2 + 1];
    let onset_env: Vec<f32> = (0..n_frames)
        .map(|i| {
            let current: Vec<f32> = short.magnitudes(x, i * HOP).iter().map(|m| (1.0 + FLUX_COMPRESSION * m).ln()).collect();
            let flux = current.iter().zip(&previous).map(|(c, p)| (c - p).max(0.0)).sum();
            previous = current;
            flux
        })
        .collect();
    let onsets = pick_onsets(&onset_env);

    let rms = (0..n_frames)
        .map(|i| {
            let lo = (i * HOP).saturating_sub(HOP);
            let hi = (i * HOP + HOP).min(x.len());
            let window = &x[lo..hi];
            (window.iter().map(|s| s * s).sum::<f32>() / window.len().max(1) as f32).sqrt()
        })
        .collect();

    Features { hop_seconds, onset_env, onsets, chroma, bass_chroma, tuning_cents, rms, sustained_partials }
}

/// See [`Features::sustained_partials`]. Peaks of each frame are sorted by frequency.
fn sustained_partial_share(peaks: &[Vec<(f32, f32)>], bin_hz: f32) -> f32 {
    let tolerance = PARTIAL_TOLERANCE_BINS * bin_hz;
    let has_peak_near = |frame: &[(f32, f32)], hz: f32| {
        let i = frame.partition_point(|p| p.0 < hz - tolerance);
        frame.get(i).is_some_and(|p| p.0 <= hz + tolerance)
    };
    let (mut held, mut total) = (0f64, 0f64);
    for (i, frame) in peaks.iter().enumerate() {
        for &(hz, mag) in frame {
            total += mag as f64;
            let inside = i >= PARTIAL_HOLD_FRAMES && i + PARTIAL_HOLD_FRAMES < peaks.len();
            if inside
                && (PARTIAL_MIN_HZ..=PARTIAL_MAX_HZ).contains(&hz)
                && (1..=PARTIAL_HOLD_FRAMES).all(|d| has_peak_near(&peaks[i - d], hz) && has_peak_near(&peaks[i + d], hz))
            {
                held += mag as f64;
            }
        }
    }
    if total > 0.0 { (held / total) as f32 } else { 0.0 }
}

/// Hann-windowed magnitude spectrum of the frame centred on a sample, scaled so a sine of
/// amplitude `a` peaks at about `a / 2`.
struct Spectrum {
    fft: std::sync::Arc<dyn realfft::RealToComplex<f32>>,
    window: Vec<f32>,
    input: Vec<f32>,
    output: Vec<realfft::num_complex::Complex<f32>>,
    magnitudes: Vec<f32>,
}

impl Spectrum {
    fn new(planner: &mut RealFftPlanner<f32>, size: usize) -> Spectrum {
        let fft = planner.plan_fft_forward(size);
        let window = (0..size).map(|j| 0.5 - 0.5 * (TAU * j as f32 / size as f32).cos()).collect();
        let (input, output) = (fft.make_input_vec(), fft.make_output_vec());
        Spectrum { fft, window, input, output, magnitudes: vec![0.0; size / 2 + 1] }
    }

    fn magnitudes(&mut self, x: &[f32], center: usize) -> &[f32] {
        let size = self.window.len();
        let start = center as isize - (size / 2) as isize;
        for (j, slot) in self.input.iter_mut().enumerate() {
            let at = start + j as isize;
            let sample = if at >= 0 && (at as usize) < x.len() { x[at as usize] } else { 0.0 };
            *slot = sample * self.window[j];
        }
        // Lengths always match the plan, so this cannot fail.
        self.fft.process(&mut self.input, &mut self.output).expect("FFT buffers match the plan");
        let scale = 4.0 / size as f32;
        for (m, c) in self.magnitudes.iter_mut().zip(&self.output) {
            *m = c.norm() * scale;
        }
        &self.magnitudes
    }
}

/// Local maxima between `min_hz` and `max_hz` as (frequency, magnitude), refined by
/// parabolic interpolation on log magnitude.
fn spectral_peaks(mags: &[f32], rate: f32, size: usize, min_hz: f32, max_hz: f32) -> Vec<(f32, f32)> {
    let bin_hz = rate / size as f32;
    let loudest = mags.iter().fold(0f32, |m, &v| m.max(v));
    let floor = (loudest * 10f32.powf(PEAK_FLOOR_DB / 20.0)).max(PEAK_ABS_FLOOR);
    let lo = ((min_hz / bin_hz).floor() as usize).max(PEAK_NEIGHBORS);
    let hi = ((max_hz / bin_hz).ceil() as usize).min(mags.len() - 1 - PEAK_NEIGHBORS);
    let mut peaks = Vec::new();
    for k in lo..=hi {
        let m = mags[k];
        if m <= floor {
            continue;
        }
        let left_ok = (k - PEAK_NEIGHBORS..k).all(|j| mags[j] < m);
        let right_ok = (k + 1..=k + PEAK_NEIGHBORS).all(|j| mags[j] <= m);
        if !(left_ok && right_ok) {
            continue;
        }
        let (a, b, c) = (mags[k - 1].max(1e-12).ln(), m.ln(), mags[k + 1].max(1e-12).ln());
        let denom = a - 2.0 * b + c;
        let p = if denom.abs() > 1e-12 { (0.5 * (a - c) / denom).clamp(-0.5, 0.5) } else { 0.0 };
        let freq = (k as f32 + p) * bin_hz;
        if (min_hz..=max_hz).contains(&freq) {
            peaks.push((freq, (b - 0.25 * (a - c) * p).exp()));
        }
    }
    peaks
}

/// Global deviation from A440 equal temperament, in cents (−50..50): magnitude-weighted
/// circular mean of every chroma-band peak's offset from its nearest semitone. Only peaks in
/// `CHROMA_MIN_HZ..=CHROMA_MAX_HZ` count; the peaks above (collected up to `PEAKS_MAX_HZ` for
/// the sustained-partial share) are skipped, so the tuning is the same as in analyzer v1.
fn estimate_tuning(peaks: &[Vec<(f32, f32)>]) -> f32 {
    let (mut re, mut im) = (0f64, 0f64);
    for &(freq, mag) in peaks.iter().flatten() {
        if !(CHROMA_MIN_HZ..=CHROMA_MAX_HZ).contains(&freq) {
            continue;
        }
        let angle = TAU as f64 * (1200.0 * (freq as f64 / 440.0).log2()) / 100.0;
        re += mag as f64 * angle.cos();
        im += mag as f64 * angle.sin();
    }
    if re == 0.0 && im == 0.0 {
        return 0.0;
    }
    (im.atan2(re) / TAU as f64 * 100.0) as f32
}

/// Sums peak magnitudes per pitch class (nearest semitone after removing the tuning offset).
fn fold_to_chroma(peaks: &[(f32, f32)], tuning_cents: f32, min_hz: f32, max_hz: f32) -> [f32; 12] {
    let mut chroma = [0f32; 12];
    for &(freq, mag) in peaks {
        if (min_hz..=max_hz).contains(&freq) {
            let midi = 69.0 + 12.0 * (freq / 440.0).log2() - tuning_cents / 100.0;
            chroma[(midi.round() as i32).rem_euclid(12) as usize] += mag;
        }
    }
    chroma
}

fn pick_onsets(env: &[f32]) -> Vec<usize> {
    let max = env.iter().fold(0f32, |m, &v| m.max(v));
    if max <= 0.0 {
        return Vec::new();
    }
    (0..env.len())
        .filter(|&i| {
            let v = env[i];
            let before = &env[i.saturating_sub(ONSET_PEAK_FRAMES)..i];
            let after = &env[i + 1..(i + 1 + ONSET_PEAK_FRAMES).min(env.len())];
            let around = &env[i.saturating_sub(ONSET_MEAN_FRAMES)..(i + 1 + ONSET_MEAN_FRAMES).min(env.len())];
            let local_mean = around.iter().sum::<f32>() / around.len() as f32;
            v >= ONSET_MIN * max
                && before.iter().all(|&b| b < v)
                && after.iter().all(|&a| a <= v)
                && v >= local_mean + ONSET_DELTA * max
        })
        .collect()
}

#[cfg(test)]
pub(crate) fn mean_chroma(frames: &[[f32; 12]]) -> [f32; 12] {
    let mut sum = [0f32; 12];
    for f in frames {
        for (s, v) in sum.iter_mut().zip(f) {
            *s += v;
        }
    }
    sum
}

#[cfg(test)]
pub(crate) fn argmax(values: &[f32; 12]) -> usize {
    (0..12).fold(0, |best, i| if values[i] > values[best] { i } else { best })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analyze::synth::{click_loop, mix, noise_hit, tone};

    #[test]
    fn chroma_peaks_on_note() {
        let f = extract(&tone(57.0, 2.0, 6));
        assert_eq!(argmax(&mean_chroma(&f.chroma)), 9);
    }

    #[test]
    fn tuning_is_estimated() {
        let f = extract(&tone(57.3, 2.0, 6));
        assert!((20.0..=40.0).contains(&f.tuning_cents), "{}", f.tuning_cents);
        assert_eq!(argmax(&mean_chroma(&f.chroma)), 9);
    }

    #[test]
    fn bass_chroma_ignores_highs() {
        let f = extract(&mix(&tone(36.0, 2.0, 6), &tone(81.0, 2.0, 6)));
        assert_eq!(argmax(&mean_chroma(&f.bass_chroma)), 0);
    }

    #[test]
    fn held_notes_have_sustained_partials_and_drums_do_not() {
        let held = extract(&tone(64.0, 2.0, 6)).sustained_partials;
        let clicks = extract(&click_loop(90.0, 8)).sustained_partials;
        let hit = extract(&noise_hit(0.5)).sustained_partials;
        assert!(held > 0.3 && clicks < 0.1 && hit < 0.1, "{held} {clicks} {hit}");
    }

    #[test]
    fn onsets_of_click_loop() {
        let f = extract(&click_loop(90.0, 8));
        assert!((7..=9).contains(&f.onsets.len()), "{:?}", f.onsets);
    }
}

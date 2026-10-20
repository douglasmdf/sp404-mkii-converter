//! Loop tempo: whole-beat candidates from the loop length, checked against the periodicity
//! of the onset envelope.
//!
//! The output is folded into a BPM range afterwards, so only the tempo *family* matters here
//! (90, 180 and 45 are the same answer). Every tempo is therefore searched within one octave,
//! `OCTAVE_LOW..2 × OCTAVE_LOW`, and scored by how well the onset autocorrelation peaks at
//! whole multiples of its beat. Sample-pack loops are cut to a power-of-two number of beats
//! (4, 8, …, 128: in the test pack 31 of 33 loops), which fixes the family from the length
//! alone; that candidate wins whenever the onsets agree with it, which removes the 4/3 and
//! 5/4 confusions of the autocorrelation peak.
//!
//! The family is all the analyzer decides. Inside a BPM range wider than an octave (78–166
//! holds both 81 and 162) the result is the octave the raw value falls in, here always
//! `OCTAVE_LOW..2·OCTAVE_LOW`. On the test pack the onsets do not settle that choice: the
//! fast pulse is weaker than the slow one in `Breaks_Before Midnight (162bpm)` and stronger
//! in `PercRythm (83bpm)`.

use super::features::Features;
use super::Estimate;

/// The search octave: every tempo is represented by its value in `OCTAVE_LOW..2·OCTAVE_LOW`.
const OCTAVE_LOW: f64 = 70.0;
/// Step of the tempo grid over the octave, in BPM, and of the final refinement.
const GRID_STEP_BPM: f64 = 0.5;
const REFINE_STEP_BPM: f64 = 0.05;
/// Whole multiples of the beat read by the family score (up to two bars)...
const FAMILY_MULTIPLES: usize = 8;
/// ...each as the autocorrelation maximum within this many frames of the multiple (onsets
/// of real loops wander by a frame or so).
const FAMILY_TOLERANCE_FRAMES: f64 = 1.0;
/// The power-of-two-beats candidate is taken when its score reaches this share of the best
/// score over the octave, and is confident at `POW2_CONFIDENT`. On both test packs the right
/// power-of-two tempo scores at least 0.52 of the best (`Pieces Sample`, Kount) and the wrong
/// one at most 0.22 (`Truth Sample`, 160 beats; `Dearly Synth Loop`, 20 beats, 0.21; `Can't
/// go Back`, 30 beats, 0.02); this is the midpoint of that gap.
const POW2_AGREE: f64 = 0.37;
const POW2_CONFIDENT: f64 = 0.75;
/// Otherwise another whole-beat candidate is taken when it reaches this share of the best
/// score and lies within `MAX_DRIFT` (relative) of the best grid tempo.
const AGREE_MIN: f64 = 0.75;
const MAX_DRIFT: f64 = 0.02;
/// Tie-break bonuses for whole-beat candidates: whole bars, else an even beat count.
const BONUS_BAR: f64 = 0.06;
const BONUS_EVEN: f64 = 0.03;
/// Fewer onsets than this cannot give a tempo.
const MIN_ONSETS: usize = 2;

/// Raw BPM (not folded into a range) of a loop of `seconds` (its untrimmed length).
pub fn estimate_bpm(features: &Features, seconds: f64) -> Option<Estimate<f32>> {
    Rhythm::new(features, seconds).bpm()
}

/// How clearly the onsets repeat at the tempo the loop length implies; see [`Rhythm::periodicity`].
pub fn periodicity(features: &Features, seconds: f64) -> f32 {
    Rhythm::new(features, seconds).periodicity()
}

/// The onset autocorrelation of one signal, built once (it is O(n²)) and read for both the
/// periodicity and the tempo.
pub struct Rhythm {
    acf: Option<Acf>,
    hop: f64,
    onsets: usize,
    seconds: f64,
    /// Family score at each grid tempo of the octave.
    grid: Vec<(f64, f64)>,
}

impl Rhythm {
    pub fn new(features: &Features, seconds: f64) -> Rhythm {
        let hop = features.hop_seconds as f64;
        let acf = Acf::new(&features.onset_env);
        let steps = (OCTAVE_LOW / GRID_STEP_BPM) as usize;
        let grid = match &acf {
            Some(a) => (0..steps).map(|i| OCTAVE_LOW + i as f64 * GRID_STEP_BPM).map(|b| (b, a.family(60.0 / (b * hop)))).collect(),
            None => Vec::new(),
        };
        Rhythm { acf, hop, onsets: features.onsets.len(), seconds, grid }
    }

    fn family(&self, bpm: f64) -> f64 {
        self.acf.as_ref().map_or(0.0, |a| a.family(60.0 / (bpm * self.hop)))
    }

    fn best(&self) -> Option<(f64, f64)> {
        self.grid.iter().copied().fold(None, |b: Option<(f64, f64)>, c| if b.is_none_or(|b| c.1 > b.1) { Some(c) } else { b })
    }

    fn median(&self) -> f64 {
        let mut scores: Vec<f64> = self.grid.iter().map(|g| g.1).collect();
        scores.sort_by(f64::total_cmp);
        scores.get(scores.len() / 2).copied().unwrap_or(0.0)
    }

    /// The tempo of `beats` beats over the loop length, folded into the octave.
    fn tempo_of(&self, beats: f64) -> Option<f64> {
        (self.seconds > 0.0).then(|| fold_octave(60.0 * beats / self.seconds))
    }

    /// How much better the onsets fit a tempo at which the length is a whole number of bars
    /// (the power-of-two-beats tempo or any multiple of 4 beats) than a typical tempo of the
    /// octave (score minus the median score), 0–1. Near 0 for one-shots and sounds without a
    /// steady pulse; for a trimmed loop, about the height of its beat autocorrelation. Whole bars
    /// rather than only powers of two: `Dearly Synth Loop` (Kount pack) is 20 beats at 130 BPM.
    /// The best one-shot of both test packs reaches 0.11 (`OP1_ReverseArp 1`).
    pub fn periodicity(&self) -> f32 {
        let Some(pow2) = self.tempo_of(4.0) else { return 0.0 };
        let last = (2.0 * OCTAVE_LOW * self.seconds / 60.0).floor() as u64;
        let bars = (4..=last)
            .step_by(4)
            .map(|beats| 60.0 * beats as f64 / self.seconds)
            .filter(|bpm| (OCTAVE_LOW..2.0 * OCTAVE_LOW).contains(bpm))
            .map(|bpm| self.family(bpm))
            .fold(self.family(pow2), f64::max);
        (bars - self.median()).clamp(0.0, 1.0) as f32
    }

    /// See [`estimate_bpm`].
    pub fn bpm(&self) -> Option<Estimate<f32>> {
        if self.onsets < MIN_ONSETS {
            return None;
        }
        let (best_bpm, best_score) = self.best().filter(|b| b.1 > 0.0)?;
        if let Some(tempo) = self.tempo_of(4.0) {
            let agreement = self.family(tempo) / best_score;
            if agreement >= POW2_AGREE {
                return Some(Estimate { value: tempo as f32, confident: agreement >= POW2_CONFIDENT });
            }
        }
        let mut chosen: Option<(f64, f64)> = None;
        let last = (2.0 * OCTAVE_LOW * self.seconds / 60.0).floor() as u64;
        for beats in 1..=last {
            let bpm = 60.0 * beats as f64 / self.seconds;
            if !(OCTAVE_LOW..2.0 * OCTAVE_LOW).contains(&bpm) || (bpm - best_bpm).abs() > MAX_DRIFT * best_bpm {
                continue;
            }
            let agreement = self.family(bpm) / best_score;
            if agreement < AGREE_MIN {
                continue;
            }
            let bonus = if beats % 4 == 0 { BONUS_BAR } else if beats % 2 == 0 { BONUS_EVEN } else { 0.0 };
            if chosen.is_none_or(|(_, s)| agreement + bonus > s) {
                chosen = Some((bpm, agreement + bonus));
            }
        }
        if let Some((bpm, _)) = chosen {
            return Some(Estimate { value: bpm as f32, confident: true });
        }
        // No whole-beat length fits: the best grid tempo, refined on a finer step.
        let steps = (GRID_STEP_BPM / REFINE_STEP_BPM) as i32;
        let refined = (-steps..=steps)
            .map(|i| best_bpm + i as f64 * REFINE_STEP_BPM)
            .map(|b| (b, self.family(b)))
            .fold((best_bpm, best_score), |b, c| if c.1 > b.1 { c } else { b });
        Some(Estimate { value: refined.0 as f32, confident: false })
    }
}

/// Doubles or halves `bpm` into `OCTAVE_LOW..2·OCTAVE_LOW`.
fn fold_octave(mut bpm: f64) -> f64 {
    while bpm < OCTAVE_LOW {
        bpm *= 2.0;
    }
    while bpm >= 2.0 * OCTAVE_LOW {
        bpm /= 2.0;
    }
    bpm
}

/// Normalized autocorrelation of the onset envelope.
struct Acf(Vec<f64>);

impl Acf {
    fn new(env: &[f32]) -> Option<Acf> {
        let n = env.len();
        let mean = env.iter().map(|&v| v as f64).sum::<f64>() / n.max(1) as f64;
        let centered: Vec<f64> = env.iter().map(|&v| v as f64 - mean).collect();
        let energy: f64 = centered.iter().map(|v| v * v).sum();
        if n < 2 || energy <= 0.0 {
            return None;
        }
        Some(Acf((0..n).map(|lag| centered[..n - lag].iter().zip(&centered[lag..]).map(|(a, b)| a * b).sum::<f64>() / energy).collect()))
    }

    /// Linearly interpolated autocorrelation; 0 past the end.
    fn at(&self, lag: f64) -> f64 {
        let i = lag.floor() as usize;
        if lag < 0.0 || i + 1 >= self.0.len() {
            return 0.0;
        }
        let t = lag - i as f64;
        self.0[i] * (1.0 - t) + self.0[i + 1] * t
    }

    /// Mean, over the whole multiples of `period` that fit in half the signal (at most
    /// `FAMILY_MULTIPLES`), of the autocorrelation maximum near each multiple. 0 when not
    /// even one beat fits.
    fn family(&self, period: f64) -> f64 {
        let limit = (self.0.len() / 2) as f64;
        let multiples = (1..=FAMILY_MULTIPLES).filter(|&k| k as f64 * period <= limit).count();
        if multiples == 0 {
            return 0.0;
        }
        let steps = (FAMILY_TOLERANCE_FRAMES * 4.0) as i32;
        let sum: f64 = (1..=multiples)
            .map(|k| {
                let center = k as f64 * period;
                (-steps..=steps).map(|d| self.at(center + d as f64 * 0.25)).fold(f64::MIN, f64::max)
            })
            .sum();
        sum / multiples as f64
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analyze::features::extract;
    use crate::analyze::synth::{click_loop, drifting_pad, noise_hit, with_tail};
    use crate::config::BpmRange;
    use crate::tags::fold_bpm;

    /// Compared in 58–115, a range narrower than an octave, so only the tempo family counts:
    /// the analyzer does not choose between t and 2t (on the test pack the onsets cannot tell
    /// `Breaks_Before Midnight (162bpm)` from `PercRythm (83bpm)`; see `tempo` module docs).
    #[test]
    fn trimmed_click_loops() {
        for (bpm, beats) in [(90.0, 16), (80.0, 64), (77.0, 30), (162.0, 32), (61.0, 64)] {
            let signal = click_loop(bpm, beats);
            let found = estimate_bpm(&extract(&signal), signal.file_seconds).unwrap_or_else(|| panic!("no BPM for {bpm}"));
            assert_eq!(fold_bpm(found.value, BpmRange::R58_115), fold_bpm(bpm, BpmRange::R58_115), "{bpm}: {}", found.value);
            assert!(found.confident, "{bpm}");
        }
    }

    #[test]
    fn untrimmed_loop_uses_autocorrelation() {
        let signal = with_tail(&click_loop(95.0, 18), 0.37);
        let found = estimate_bpm(&extract(&signal), signal.file_seconds).expect("a BPM");
        assert_eq!(fold_bpm(found.value, BpmRange::R78_166), 95, "{}", found.value);
    }

    #[test]
    fn single_hit_has_no_bpm() {
        let signal = noise_hit(0.3);
        let found = estimate_bpm(&extract(&signal), signal.file_seconds);
        assert!(found.as_ref().is_none_or(|f| !f.confident), "{found:?}");
    }

    #[test]
    fn whole_bars_that_are_not_a_power_of_two_are_periodic() {
        // 20 beats (5 bars): the power-of-two tempo of this length is 104, not 130.
        let clicks = click_loop(130.0, 20);
        assert!(periodicity(&extract(&clicks), clicks.file_seconds) > 0.3);
    }

    /// The whole-bar search tries about a hundred tempos on a two-minute sound; a sound with no
    /// pulse must still score under the loop classifier's `PULSE_NONE` (0.06).
    #[test]
    fn long_pad_without_pulse_is_not_periodic() {
        for seconds in [60.0, 90.0, 120.0] {
            let pad = drifting_pad(seconds);
            let p = periodicity(&extract(&pad), pad.file_seconds);
            assert!(p < 0.06, "{seconds} s: {p}");
        }
    }

    #[test]
    fn click_loop_is_periodic_and_a_hit_is_not() {
        let (clicks, hit) = (click_loop(90.0, 16), noise_hit(0.3));
        let periodic = periodicity(&extract(&clicks), clicks.file_seconds);
        let single = periodicity(&extract(&hit), hit.file_seconds);
        assert!(periodic > 0.5 && single < 0.2, "{periodic} {single}");
    }
}

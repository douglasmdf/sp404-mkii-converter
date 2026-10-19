//! Loop tempo: whole-beat candidates from the loop length, checked against the periodicity
//! of the onset envelope.

use super::features::Features;
use super::Estimate;

const MIN_BPM: f64 = 40.0;
const MAX_BPM: f64 = 300.0;
/// Multiples of a beat period checked by the comb (and as many half-beat points subtracted).
const COMB_MULTIPLES: usize = 4;
/// Step of the period search, in frames.
const GRID_STEP_FRAMES: f64 = 0.05;
/// The autocorrelation period is refined on peaks up to this many beats away.
const REFINE_MAX_MULTIPLE: usize = 16;
/// A length candidate agrees with the onsets at this share of the best comb score...
const AGREE_MIN: f64 = 0.75;
/// ...where a match at half or double the tempo counts this much of a direct match...
const FAMILY_WEIGHT: f64 = 0.9;
/// ...and its beat count is within this many beats of the loop length divided by the
/// (refined) autocorrelation period, at the same, half or double tempo.
const MAX_DRIFT_BEATS: f64 = 0.1;
/// Score bonuses: whole bars (beats multiple of 4), else an even beat count; near-integer BPM.
const BONUS_BAR: f64 = 0.06;
const BONUS_EVEN: f64 = 0.03;
const BONUS_INTEGER: f64 = 0.04;
const INTEGER_TOLERANCE: f64 = 0.05;
/// Fewer onsets than this cannot give a tempo.
const MIN_ONSETS: usize = 2;

/// Raw BPM (not folded into a range). Prefers `60 × beats / seconds` for a whole number of
/// beats that the onset periodicity agrees with (confident); otherwise the periodicity alone.
pub fn estimate_bpm(features: &Features, seconds: f64) -> Option<Estimate<f32>> {
    if features.onsets.len() < MIN_ONSETS {
        return None;
    }
    let comb = Comb::new(features)?;
    let (best_period, best_score) = comb.best_period()?;
    let hop = features.hop_seconds as f64;
    let period_of = |bpm: f64| 60.0 / (bpm * hop);
    let acf_bpm = 60.0 / (comb.refine(best_period) * hop);

    let mut chosen: Option<(f64, f64)> = None;
    let first = (MIN_BPM * seconds / 60.0).ceil().max(1.0) as u64;
    let last = (MAX_BPM * seconds / 60.0).floor() as u64;
    for beats in first..=last {
        let bpm = 60.0 * beats as f64 / seconds;
        let direct = comb.score(period_of(bpm)) / best_score;
        let family = comb.score(period_of(bpm / 2.0)).max(comb.score(period_of(bpm * 2.0))) / best_score;
        let agreement = direct.max(FAMILY_WEIGHT * family);
        let drift = [0.5, 1.0, 2.0].map(|f| (beats as f64 - seconds * acf_bpm * f / 60.0).abs()).into_iter().fold(f64::MAX, f64::min);
        if agreement < AGREE_MIN || drift > MAX_DRIFT_BEATS {
            continue;
        }
        let bar_bonus = if beats % 4 == 0 { BONUS_BAR } else if beats % 2 == 0 { BONUS_EVEN } else { 0.0 };
        let integer_bonus = if (bpm - bpm.round()).abs() <= INTEGER_TOLERANCE { BONUS_INTEGER } else { 0.0 };
        let score = agreement + bar_bonus + integer_bonus;
        if chosen.is_none_or(|(_, s)| score > s) {
            chosen = Some((bpm, score));
        }
    }
    Some(match chosen {
        Some((bpm, _)) => Estimate { value: bpm as f32, confident: true },
        None => Estimate { value: acf_bpm as f32, confident: false },
    })
}

/// How periodic the onsets are: the best comb score of the onset autocorrelation, 0–1.
pub fn periodicity(features: &Features) -> f32 {
    Comb::new(features).and_then(|c| c.best_period()).map_or(0.0, |(_, s)| s.clamp(0.0, 1.0) as f32)
}

/// Normalized autocorrelation of the onset envelope, read through a beat comb.
struct Comb {
    acf: Vec<f64>,
    min_period: f64,
    max_period: f64,
}

impl Comb {
    fn new(features: &Features) -> Option<Comb> {
        let env = &features.onset_env;
        let n = env.len();
        let mean = env.iter().map(|&v| v as f64).sum::<f64>() / n.max(1) as f64;
        let centered: Vec<f64> = env.iter().map(|&v| v as f64 - mean).collect();
        let energy: f64 = centered.iter().map(|v| v * v).sum();
        if n < 2 || energy <= 0.0 {
            return None;
        }
        let acf = (0..n).map(|lag| centered[..n - lag].iter().zip(&centered[lag..]).map(|(a, b)| a * b).sum::<f64>() / energy).collect();
        let hop = features.hop_seconds as f64;
        Some(Comb { acf, min_period: 60.0 / (MAX_BPM * hop), max_period: 60.0 / (MIN_BPM * hop) })
    }

    /// Linearly interpolated autocorrelation; 0 past the end.
    fn at(&self, lag: f64) -> f64 {
        let i = lag.floor() as usize;
        if lag < 0.0 || i + 1 >= self.acf.len() {
            return 0.0;
        }
        let t = lag - i as f64;
        self.acf[i] * (1.0 - t) + self.acf[i + 1] * t
    }

    /// Mean autocorrelation at whole multiples of `period` minus the mean at the half-way
    /// points, so half and double tempo score lower than the beat itself.
    fn score(&self, period: f64) -> f64 {
        let limit = (self.acf.len() - 1) as f64;
        let multiples = (1..=COMB_MULTIPLES).filter(|&k| k as f64 * period <= limit).count();
        if multiples == 0 {
            return 0.0;
        }
        let on: f64 = (1..=multiples).map(|k| self.at(k as f64 * period)).sum();
        let off: f64 = (1..=multiples).map(|k| self.at((k as f64 - 0.5) * period)).sum();
        ((on - off) / multiples as f64).max(0.0)
    }

    fn best_period(&self) -> Option<(f64, f64)> {
        let steps = ((self.max_period - self.min_period) / GRID_STEP_FRAMES) as usize;
        let best = (0..=steps)
            .map(|s| self.min_period + s as f64 * GRID_STEP_FRAMES)
            .map(|p| (p, self.score(p)))
            .fold(None, |best: Option<(f64, f64)>, c| if best.is_none_or(|b| c.1 > b.1) { Some(c) } else { best });
        best.filter(|b| b.1 > 0.0)
    }

    /// Sharpens `period` on autocorrelation peaks at 2, 4, 8... beats (parabolic interpolation).
    fn refine(&self, period: f64) -> f64 {
        let limit = (self.acf.len() - 1) as f64 / 2.0;
        let mut refined = period;
        let mut k = 2;
        while k <= REFINE_MAX_MULTIPLE && k as f64 * refined <= limit {
            let center = k as f64 * refined;
            let reach = (refined / 4.0).max(2.0);
            let lo = (center - reach).floor().max(1.0) as usize;
            let hi = ((center + reach).ceil() as usize).min(self.acf.len() - 2);
            let peak = (lo..=hi).fold(lo, |best, i| if self.acf[i] > self.acf[best] { i } else { best });
            let (a, b, c) = (self.acf[peak - 1], self.acf[peak], self.acf[peak + 1]);
            let denom = a - 2.0 * b + c;
            let offset = if denom.abs() > 1e-12 { (0.5 * (a - c) / denom).clamp(-0.5, 0.5) } else { 0.0 };
            refined = (peak as f64 + offset) / k as f64;
            k *= 2;
        }
        refined
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analyze::features::extract;
    use crate::analyze::synth::{click_loop, noise_hit, with_tail};
    use crate::config::BpmRange;
    use crate::tags::fold_bpm;

    #[test]
    fn trimmed_click_loops() {
        for (bpm, beats) in [(90.0, 16), (80.0, 64), (77.0, 30), (162.0, 32), (61.0, 64)] {
            let signal = click_loop(bpm, beats);
            let found = estimate_bpm(&extract(&signal), signal.source_seconds).unwrap_or_else(|| panic!("no BPM for {bpm}"));
            assert_eq!(fold_bpm(found.value, BpmRange::R78_166), fold_bpm(bpm, BpmRange::R78_166), "{bpm}: {}", found.value);
            assert!(found.confident, "{bpm}");
        }
    }

    #[test]
    fn untrimmed_loop_uses_autocorrelation() {
        let signal = with_tail(&click_loop(95.0, 18), 0.37);
        let found = estimate_bpm(&extract(&signal), signal.source_seconds).expect("a BPM");
        assert_eq!(fold_bpm(found.value, BpmRange::R78_166), 95, "{}", found.value);
    }

    #[test]
    fn single_hit_has_no_bpm() {
        let signal = noise_hit(0.3);
        let found = estimate_bpm(&extract(&signal), signal.source_seconds);
        assert!(found.as_ref().is_none_or(|f| !f.confident), "{found:?}");
    }

    #[test]
    fn click_loop_is_periodic_and_a_hit_is_not() {
        let periodic = periodicity(&extract(&click_loop(90.0, 16)));
        let single = periodicity(&extract(&noise_hit(0.3)));
        assert!(periodic > 0.5 && single < 0.2, "{periodic} {single}");
    }
}

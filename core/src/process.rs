//! Turns decoded audio into SP-404 MKII samples: ≤2 channels, 48 kHz, 16-bit.

use rubato::{Resampler, SincFixedIn, SincInterpolationParameters, SincInterpolationType, WindowFunction};

use crate::decode::{AudioBuffer, DecodedAudio};
use crate::error::{CoreError, Result};

pub const TARGET_RATE: u32 = 48_000;
const RESAMPLE_CHUNK: usize = 1024;

#[derive(Debug, Clone, PartialEq)]
pub struct Quantized {
    pub interleaved: Vec<i16>,
    pub clipped_samples: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Processed {
    pub interleaved: Vec<i16>,
    pub channels: u16,
    pub clipped_samples: u64,
    pub downmixed_from: Option<usize>,
}

pub fn process(decoded: DecodedAudio, downmix_multichannel: bool) -> Result<Processed> {
    let DecodedAudio { buffer, exact_16bit, .. } = decoded;
    let AudioBuffer { mut channels, sample_rate } = buffer;

    let mut downmixed_from = None;
    if channels.len() > 2 {
        if !downmix_multichannel {
            return Err(CoreError::TooManyChannels(channels.len()));
        }
        downmixed_from = Some(channels.len());
        channels = downmix_to_stereo(&channels);
    }

    let resampled = sample_rate != TARGET_RATE;
    if resampled {
        channels = resample(&channels, sample_rate, TARGET_RATE)?;
    }

    let dither = !(exact_16bit && !resampled && downmixed_from.is_none());
    let quantized = quantize(&channels, dither);
    Ok(Processed {
        interleaved: quantized.interleaved,
        channels: channels.len() as u16,
        clipped_samples: quantized.clipped_samples,
        downmixed_from,
    })
}

/// 6 channels are treated as standard 5.1 (FL, FR, FC, LFE, SL, SR): ITU weights, LFE
/// dropped, normalized so a full-scale signal cannot clip. Any other layout: even-indexed
/// channels are averaged into left, odd-indexed into right.
pub fn downmix_to_stereo(channels: &[Vec<f32>]) -> Vec<Vec<f32>> {
    let frames = channels.first().map_or(0, Vec::len);
    if channels.len() == 6 {
        let c = std::f32::consts::FRAC_1_SQRT_2;
        let norm = 1.0 / (1.0 + 2.0 * c);
        let (fl, fr, fc, sl, sr) = (&channels[0], &channels[1], &channels[2], &channels[4], &channels[5]);
        let left = (0..frames).map(|i| (fl[i] + c * fc[i] + c * sl[i]) * norm).collect();
        let right = (0..frames).map(|i| (fr[i] + c * fc[i] + c * sr[i]) * norm).collect();
        return vec![left, right];
    }
    let mut left = vec![0.0f32; frames];
    let mut right = vec![0.0f32; frames];
    let (mut n_left, mut n_right) = (0usize, 0usize);
    for (index, channel) in channels.iter().enumerate() {
        let target = if index % 2 == 0 {
            n_left += 1;
            &mut left
        } else {
            n_right += 1;
            &mut right
        };
        for (t, s) in target.iter_mut().zip(channel) {
            *t += s;
        }
    }
    for s in left.iter_mut() {
        *s /= n_left.max(1) as f32;
    }
    for s in right.iter_mut() {
        *s /= n_right.max(1) as f32;
    }
    vec![left, right]
}

pub fn resample(channels: &[Vec<f32>], from_rate: u32, to_rate: u32) -> Result<Vec<Vec<f32>>> {
    let frames = channels.first().map_or(0, Vec::len);
    if frames == 0 || from_rate == to_rate {
        return Ok(channels.to_vec());
    }
    let ratio = to_rate as f64 / from_rate as f64;
    let delay = measure_delay(ratio)?;
    let expected = (frames as f64 * ratio).round() as usize;
    let mut out = run_resampler(new_resampler(ratio, channels.len())?, channels, expected + delay)?;
    for channel in out.iter_mut() {
        channel.drain(..delay);
        channel.truncate(expected);
    }
    Ok(out)
}

fn new_resampler(ratio: f64, n_channels: usize) -> Result<SincFixedIn<f32>> {
    let params = SincInterpolationParameters {
        sinc_len: 256,
        f_cutoff: 0.95,
        interpolation: SincInterpolationType::Linear,
        oversampling_factor: 256,
        window: WindowFunction::BlackmanHarris2,
    };
    SincFixedIn::<f32>::new(ratio, 1.0, params, RESAMPLE_CHUNK, n_channels)
        .map_err(|e| CoreError::Resample(e.to_string()))
}

/// Feeds all input through the resampler, then flushes until at least `min_len` frames are out.
fn run_resampler(mut resampler: SincFixedIn<f32>, channels: &[Vec<f32>], min_len: usize) -> Result<Vec<Vec<f32>>> {
    let frames = channels.first().map_or(0, Vec::len);
    let mut out: Vec<Vec<f32>> = vec![Vec::with_capacity(min_len + 2 * RESAMPLE_CHUNK); channels.len()];
    let mut pos = 0;
    while frames - pos >= RESAMPLE_CHUNK {
        let block: Vec<&[f32]> = channels.iter().map(|c| &c[pos..pos + RESAMPLE_CHUNK]).collect();
        let produced = resampler.process(&block, None).map_err(|e| CoreError::Resample(e.to_string()))?;
        append(&mut out, produced);
        pos += RESAMPLE_CHUNK;
    }
    if pos < frames {
        let block: Vec<&[f32]> = channels.iter().map(|c| &c[pos..]).collect();
        let produced = resampler
            .process_partial(Some(block.as_slice()), None)
            .map_err(|e| CoreError::Resample(e.to_string()))?;
        append(&mut out, produced);
    }
    while out[0].len() < min_len {
        let produced = resampler
            .process_partial(None::<&[Vec<f32>]>, None)
            .map_err(|e| CoreError::Resample(e.to_string()))?;
        append(&mut out, produced);
    }
    Ok(out)
}

/// The resampler's real latency in output frames, measured by sending an impulse through
/// an identical resampler. `output_delay()` does not match rubato 0.15's actual alignment
/// (it already compensates part of the filter delay internally), so we measure instead.
fn measure_delay(ratio: f64) -> Result<usize> {
    let position = 2 * RESAMPLE_CHUNK;
    let mut impulse = vec![0.0f32; 2 * position];
    impulse[position] = 1.0;
    let min_len = ((2 * position) as f64 * ratio).ceil() as usize + RESAMPLE_CHUNK;
    let out = run_resampler(new_resampler(ratio, 1)?, &[impulse], min_len)?;
    let peak = out[0]
        .iter()
        .enumerate()
        .fold((0usize, 0f32), |best, (i, v)| if v.abs() > best.1 { (i, v.abs()) } else { best })
        .0;
    Ok((peak as f64 - position as f64 * ratio).round().max(0.0) as usize)
}

fn append(out: &mut [Vec<f32>], produced: Vec<Vec<f32>>) {
    for (dst, src) in out.iter_mut().zip(produced) {
        dst.extend(src);
    }
}

/// Scales to 16-bit, optionally adding TPDF dither (±1 LSB), and clamps.
/// `clipped_samples` counts samples beyond ±1.0 (above 0 dBFS) before quantizing.
pub fn quantize(channels: &[Vec<f32>], dither: bool) -> Quantized {
    let frames = channels.first().map_or(0, Vec::len);
    let mut rng = XorShift64(0x9E37_79B9_7F4A_7C15);
    let mut interleaved = Vec::with_capacity(frames * channels.len());
    let mut clipped_samples = 0u64;
    for i in 0..frames {
        for channel in channels {
            let x = channel[i];
            if x > 1.0 || x < -1.0 {
                clipped_samples += 1;
            }
            let mut v = x as f64 * 32768.0;
            if dither {
                v += rng.next_f64() - rng.next_f64();
            }
            interleaved.push(v.round().clamp(-32768.0, 32767.0) as i16);
        }
    }
    Quantized { interleaved, clipped_samples }
}

/// Small deterministic PRNG for dither (same input → same output file).
struct XorShift64(u64);

impl XorShift64 {
    fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }

    /// Uniform in [0, 1).
    fn next_f64(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::decode::{AudioBuffer, DecodedAudio};

    fn sine(freq: f32, rate: u32, frames: usize, amp: f32) -> Vec<f32> {
        (0..frames)
            .map(|i| amp * (2.0 * std::f32::consts::PI * freq * i as f32 / rate as f32).sin())
            .collect()
    }

    fn rising_zero_crossings(x: &[f32]) -> usize {
        x.windows(2).filter(|w| w[0] < 0.0 && w[1] >= 0.0).count()
    }

    fn rms(x: &[f32]) -> f32 {
        (x.iter().map(|s| s * s).sum::<f32>() / x.len() as f32).sqrt()
    }

    #[test]
    fn resample_keeps_length_frequency_level_and_alignment() {
        let input = sine(1000.0, 44_100, 44_100, 0.5);
        let out = resample(&[input], 44_100, 48_000).unwrap();
        assert_eq!(out[0].len(), 48_000);
        let middle = &out[0][4_800..43_200]; // 0.8 s
        let freq = rising_zero_crossings(middle) as f32 / 0.8;
        assert!((freq - 1000.0).abs() <= 2.0, "frequency {freq}");
        let level_db = 20.0 * (rms(middle) / (0.5 / 2f32.sqrt())).log10();
        assert!(level_db.abs() < 0.1, "level {level_db} dB");
        let ideal = sine(1000.0, 48_000, 48_000, 0.5);
        let max_err = (4_800..43_200).map(|i| (out[0][i] - ideal[i]).abs()).fold(0f32, f32::max);
        assert!(max_err < 0.05, "misaligned, max error {max_err}");
    }

    #[test]
    fn resample_aligns_for_other_common_rates() {
        for from in [96_000u32, 22_050, 88_200, 32_000] {
            let input = sine(1000.0, from, from as usize, 0.5);
            let out = resample(&[input], from, 48_000).unwrap();
            assert_eq!(out[0].len(), 48_000, "{from}");
            let ideal = sine(1000.0, 48_000, 48_000, 0.5);
            let max_err = (4_800..43_200).map(|i| (out[0][i] - ideal[i]).abs()).fold(0f32, f32::max);
            assert!(max_err < 0.05, "{from}: misaligned, max error {max_err}");
        }
    }

    #[test]
    fn resample_short_and_empty_inputs() {
        // Review Focus 4
        let out = resample(&[vec![0.1; 10], vec![0.1; 10]], 44_100, 48_000).unwrap();
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].len(), 11);
        assert_eq!(out[1].len(), 11);
        let empty = resample(&[Vec::new()], 44_100, 48_000).unwrap();
        assert_eq!(empty, vec![Vec::<f32>::new()]);
    }

    #[test]
    fn resample_same_rate_is_identity() {
        let x = vec![vec![0.1, -0.2, 0.3]];
        assert_eq!(resample(&x, 48_000, 48_000).unwrap(), x);
    }

    #[test]
    fn quantize_without_dither_is_exact() {
        let ch = vec![vec![0.0, 1.0 / 32768.0, -1.0, 32767.0 / 32768.0]];
        let q = quantize(&ch, false);
        assert_eq!(q.interleaved, vec![0, 1, -32768, 32767]);
        assert_eq!(q.clipped_samples, 0);
    }

    #[test]
    fn quantize_interleaves_channels() {
        let ch = vec![vec![0.0, 2.0 / 32768.0], vec![1.0 / 32768.0, 3.0 / 32768.0]];
        assert_eq!(quantize(&ch, false).interleaved, vec![0, 1, 2, 3]);
    }

    #[test]
    fn quantize_counts_and_clamps_overs() {
        let q = quantize(&[vec![1.5, -1.5, 1.0]], false);
        assert_eq!(q.interleaved, vec![32767, -32768, 32767]);
        assert_eq!(q.clipped_samples, 2);
    }

    #[test]
    fn dither_stays_within_one_lsb_and_is_unbiased() {
        let x: Vec<f32> = (0..48_000).map(|i| 0.25 * (i as f32 * 0.001).sin()).collect();
        let q = quantize(&[x.clone()], true);
        let mut sum = 0.0f64;
        let mut differs = false;
        for (s, v) in q.interleaved.iter().zip(&x) {
            let exact = *v as f64 * 32768.0;
            let err = *s as f64 - exact;
            assert!(err.abs() <= 1.5, "error {err}");
            sum += err;
            differs |= *s != exact.round() as i16;
        }
        assert!((sum / x.len() as f64).abs() < 0.05);
        assert!(differs, "dither was not applied");
    }

    #[test]
    fn downmix_51_uses_itu_weights_and_drops_lfe() {
        let c = std::f32::consts::FRAC_1_SQRT_2;
        let norm = 1.0 / (1.0 + 2.0 * c);
        let center_only = vec![vec![0.0], vec![0.0], vec![1.0], vec![0.0], vec![0.0], vec![0.0]];
        let out = downmix_to_stereo(&center_only);
        assert!((out[0][0] - c * norm).abs() < 1e-6);
        assert!((out[1][0] - c * norm).abs() < 1e-6);
        let lfe_only = vec![vec![0.0], vec![0.0], vec![0.0], vec![1.0], vec![0.0], vec![0.0]];
        assert_eq!(downmix_to_stereo(&lfe_only), vec![vec![0.0], vec![0.0]]);
    }

    #[test]
    fn downmix_generic_averages_even_and_odd_channels() {
        let four = vec![vec![1.0], vec![0.0], vec![0.5], vec![0.0]];
        assert_eq!(downmix_to_stereo(&four), vec![vec![0.75], vec![0.0]]);
    }

    fn decoded(channels: Vec<Vec<f32>>, sample_rate: u32, exact_16bit: bool) -> DecodedAudio {
        DecodedAudio { buffer: AudioBuffer { channels, sample_rate }, exact_16bit, truncated: false }
    }

    #[test]
    fn process_passthrough_is_bit_exact() {
        // Review Focus 5
        let samples: Vec<i16> = (0..2000i32).map(|i| ((i * 37) % 65536 - 32768) as i16).collect();
        let left: Vec<f32> = samples.iter().step_by(2).map(|&s| s as f32 / 32768.0).collect();
        let right: Vec<f32> = samples.iter().skip(1).step_by(2).map(|&s| s as f32 / 32768.0).collect();
        let p = process(decoded(vec![left, right], 48_000, true), true).unwrap();
        assert_eq!(p.channels, 2);
        assert_eq!(p.interleaved, samples);
        assert_eq!(p.clipped_samples, 0);
        assert_eq!(p.downmixed_from, None);
    }

    #[test]
    fn process_rejects_multichannel_when_downmix_off() {
        let six = vec![vec![0.0; 4]; 6];
        assert!(matches!(
            process(decoded(six, 48_000, true), false),
            Err(CoreError::TooManyChannels(6))
        ));
    }

    #[test]
    fn process_downmixes_and_reports() {
        let six = vec![vec![0.1; 4]; 6];
        let p = process(decoded(six, 48_000, true), true).unwrap();
        assert_eq!(p.channels, 2);
        assert_eq!(p.downmixed_from, Some(6));
        assert_eq!(p.interleaved.len(), 8);
    }

    #[test]
    fn process_mono_stays_mono_and_resamples() {
        let p = process(decoded(vec![sine(440.0, 44_100, 441, 0.5)], 44_100, true), true).unwrap();
        assert_eq!(p.channels, 1);
        assert_eq!(p.interleaved.len(), 480);
    }
}

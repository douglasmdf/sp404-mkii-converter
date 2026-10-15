//! Reads any supported audio file into planar f32 samples (symphonia).

use std::fs::File;
use std::path::Path;

use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::{CodecParameters, DecoderOptions, CODEC_TYPE_NULL};
use symphonia::core::errors::Error as SymphoniaError;
use symphonia::core::formats::{FormatOptions, FormatReader};
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;
use symphonia::core::sample::SampleFormat;

use crate::error::{io_err, CoreError, Result};

#[derive(Debug, Clone, PartialEq)]
pub struct AudioBuffer {
    pub channels: Vec<Vec<f32>>,
    pub sample_rate: u32,
}

impl AudioBuffer {
    pub fn frames(&self) -> usize {
        self.channels.first().map_or(0, Vec::len)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct DecodedAudio {
    pub buffer: AudioBuffer,
    /// True when the source is integer PCM of 16 bits or less, so converting back to
    /// 16 bits without resampling loses nothing and needs no dither.
    pub exact_16bit: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AudioInfo {
    pub frames: Option<u64>,
    pub sample_rate: Option<u32>,
    pub channels: Option<usize>,
}

pub fn decode_file(path: &Path) -> Result<DecodedAudio> {
    let (mut format, track_id, params) = open(path)?;
    let mut decoder = symphonia::default::get_codecs()
        .make(&params, &DecoderOptions::default())
        .map_err(map_err)?;
    let mut sample_rate = params.sample_rate.unwrap_or(0);
    let mut channels: Vec<Vec<f32>> = vec![Vec::new(); params.channels.map_or(0, |c| c.count())];

    loop {
        let packet = match format.next_packet() {
            Ok(packet) => packet,
            Err(SymphoniaError::IoError(e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
            Err(SymphoniaError::ResetRequired) => break,
            Err(e) => return Err(map_err(e)),
        };
        if packet.track_id() != track_id {
            continue;
        }
        let decoded = match decoder.decode(&packet) {
            Ok(decoded) => decoded,
            Err(SymphoniaError::DecodeError(_)) => continue,
            Err(e) => return Err(map_err(e)),
        };
        let spec = *decoded.spec();
        let n_ch = spec.channels.count();
        if channels.len() != n_ch {
            if channels.iter().any(|c| !c.is_empty()) {
                return Err(CoreError::Decode("channel count changed mid-stream".into()));
            }
            channels = vec![Vec::new(); n_ch];
        }
        sample_rate = spec.rate;
        let mut samples = SampleBuffer::<f32>::new(decoded.capacity() as u64, spec);
        samples.copy_interleaved_ref(decoded);
        for (i, s) in samples.samples().iter().enumerate() {
            channels[i % n_ch].push(*s);
        }
    }

    if channels.is_empty() || sample_rate == 0 {
        return Err(CoreError::Decode("no audio channels found".into()));
    }
    let exact_16bit = matches!(params.bits_per_sample, Some(bits) if bits <= 16)
        && !matches!(params.sample_format, Some(SampleFormat::F32 | SampleFormat::F64));
    Ok(DecodedAudio { buffer: AudioBuffer { channels, sample_rate }, exact_16bit })
}

pub fn probe_info(path: &Path) -> Result<AudioInfo> {
    let (_, _, params) = open(path)?;
    Ok(AudioInfo {
        frames: params.n_frames,
        sample_rate: params.sample_rate,
        channels: params.channels.map(|c| c.count()),
    })
}

fn open(path: &Path) -> Result<(Box<dyn FormatReader>, u32, CodecParameters)> {
    let file = File::open(path).map_err(io_err(path))?;
    let stream = MediaSourceStream::new(Box::new(file), Default::default());
    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }
    let probed = symphonia::default::get_probe()
        .format(&hint, stream, &FormatOptions::default(), &MetadataOptions::default())
        .map_err(map_err)?;
    let format = probed.format;
    let track = format
        .tracks()
        .iter()
        .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
        .ok_or_else(|| CoreError::Decode("no audio track".into()))?;
    let (track_id, params) = (track.id, track.codec_params.clone());
    Ok((format, track_id, params))
}

fn map_err(error: SymphoniaError) -> CoreError {
    match error {
        SymphoniaError::Unsupported(_) => CoreError::UnsupportedFormat,
        other => CoreError::Decode(other.to_string()),
    }
}

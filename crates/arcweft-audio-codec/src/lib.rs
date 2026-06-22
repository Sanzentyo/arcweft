//! Encoded audio bytes to mono/stereo interleaved `f32` PCM.
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::similar_names,
    clippy::too_many_lines
)]

use arcweft_audio_core::{AudioFormat, DecodedAudio};
use std::io::{Cursor, ErrorKind};
use symphonia::core::codecs::audio::{AudioDecoderOptions, CODEC_ID_NULL_AUDIO};
use symphonia::core::errors::Error as SymphoniaError;
use symphonia::core::formats::probe::Hint;
use symphonia::core::formats::{FormatOptions, TrackType};
use symphonia::core::io::{MediaSourceStream, MediaSourceStreamOptions};
use symphonia::core::meta::MetadataOptions;
use thiserror::Error;

#[derive(Clone, Copy, Debug)]
pub struct AudioDecodeLimits {
    pub max_input_bytes: usize,
    pub max_decoded_frames: u64,
}

impl Default for AudioDecodeLimits {
    fn default() -> Self {
        Self {
            max_input_bytes: 256 * 1024 * 1024,
            max_decoded_frames: 48_000 * 60 * 30,
        }
    }
}

pub fn decode_audio(
    bytes: &[u8],
    format: AudioFormat,
    limits: AudioDecodeLimits,
) -> Result<DecodedAudio, AudioDecodeError> {
    if bytes.len() > limits.max_input_bytes {
        return Err(AudioDecodeError::Limit(format!(
            "encoded audio has {} bytes; maximum is {}",
            bytes.len(),
            limits.max_input_bytes
        )));
    }

    let stream = MediaSourceStream::new(
        Box::new(Cursor::new(bytes.to_vec())),
        MediaSourceStreamOptions::default(),
    );
    let mut hint = Hint::new();
    match format {
        AudioFormat::Wav => hint.with_extension("wav"),
        AudioFormat::Flac => hint.with_extension("flac"),
        AudioFormat::OggVorbis => hint.with_extension("ogg"),
        AudioFormat::Mp3 => hint.with_extension("mp3"),
        AudioFormat::AacMp4 => hint.with_extension("m4a"),
    };
    let mut reader = symphonia::default::get_probe()
        .probe(
            &hint,
            stream,
            FormatOptions::default(),
            MetadataOptions::default(),
        )
        .map_err(|error| AudioDecodeError::Probe(error.to_string()))?;
    let track = reader
        .default_track(TrackType::Audio)
        .ok_or(AudioDecodeError::MissingTrack)?;
    let codec_params = track
        .codec_params
        .as_ref()
        .and_then(|params| params.audio())
        .ok_or(AudioDecodeError::UnsupportedCodec)?;
    if codec_params.codec == CODEC_ID_NULL_AUDIO {
        return Err(AudioDecodeError::UnsupportedCodec);
    }
    let track_id = track.id;
    let mut decoder = symphonia::default::get_codecs()
        .make_audio_decoder(codec_params, &AudioDecoderOptions::default())
        .map_err(|error| AudioDecodeError::Decoder(error.to_string()))?;
    let mut output = Vec::new();
    let mut sample_rate_hz = None;
    let mut channels = None;

    loop {
        let packet = match reader.next_packet() {
            Ok(Some(packet)) => packet,
            Ok(None) => break,
            Err(SymphoniaError::IoError(error)) if error.kind() == ErrorKind::UnexpectedEof => {
                break;
            }
            Err(SymphoniaError::ResetRequired) => {
                return Err(AudioDecodeError::ResetRequired);
            }
            Err(error) => return Err(AudioDecodeError::Packet(error.to_string())),
        };
        if packet.track_id != track_id {
            continue;
        }
        let decoded = match decoder.decode(&packet) {
            Ok(decoded) => decoded,
            Err(SymphoniaError::DecodeError(_)) => continue,
            Err(SymphoniaError::IoError(error)) if error.kind() == ErrorKind::UnexpectedEof => {
                break;
            }
            Err(error) => return Err(AudioDecodeError::Decoder(error.to_string())),
        };
        let specification = decoded.spec();
        let decoded_channels = specification.channels().count();
        if !(1..=2).contains(&decoded_channels) {
            return Err(AudioDecodeError::UnsupportedChannels(decoded_channels));
        }
        if let Some(expected) = channels {
            if expected != decoded_channels as u16 {
                return Err(AudioDecodeError::StreamChanged(
                    "channel count changed while decoding".to_owned(),
                ));
            }
        } else {
            channels = Some(decoded_channels as u16);
        }
        if let Some(expected) = sample_rate_hz {
            if expected != specification.rate() {
                return Err(AudioDecodeError::StreamChanged(
                    "sample rate changed while decoding".to_owned(),
                ));
            }
        } else {
            sample_rate_hz = Some(specification.rate());
        }

        let mut samples = vec![0.0; decoded.samples_interleaved()];
        decoded.copy_to_slice_interleaved(&mut samples);
        output.extend_from_slice(&samples);
        let decoded_frames = output.len() as u64 / decoded_channels as u64;
        if decoded_frames > limits.max_decoded_frames {
            return Err(AudioDecodeError::Limit(format!(
                "decoded audio has more than {} frames",
                limits.max_decoded_frames
            )));
        }
    }

    let sample_rate_hz = sample_rate_hz.ok_or(AudioDecodeError::Empty)?;
    let channels = channels.ok_or(AudioDecodeError::Empty)?;
    DecodedAudio::new(sample_rate_hz, channels, output)
        .map_err(|error| AudioDecodeError::InvalidPcm(error.to_string()))
}

/// Resamples complete decoded assets away from the audio callback.
///
/// The cubic implementation is deterministic and dependency-light. Product
/// profiles may substitute a band-limited resampler behind this trait without
/// changing the mixer or command API.
pub trait AudioResampler {
    fn resample(
        &self,
        input: &DecodedAudio,
        output_sample_rate_hz: u32,
    ) -> Result<DecodedAudio, AudioDecodeError>;
}

#[derive(Clone, Copy, Debug, Default)]
pub struct CubicResampler;

impl AudioResampler for CubicResampler {
    fn resample(
        &self,
        input: &DecodedAudio,
        output_sample_rate_hz: u32,
    ) -> Result<DecodedAudio, AudioDecodeError> {
        if output_sample_rate_hz == 0 {
            return Err(AudioDecodeError::InvalidPcm(
                "output sample rate must be non-zero".to_owned(),
            ));
        }
        if input.sample_rate_hz() == output_sample_rate_hz {
            return DecodedAudio::new(
                input.sample_rate_hz(),
                input.channels(),
                input.samples().to_vec(),
            )
            .map_err(|error| AudioDecodeError::InvalidPcm(error.to_string()));
        }
        let channels = usize::from(input.channels());
        let input_frames = input.frame_count() as usize;
        let output_frames = ((input_frames as u128 * u128::from(output_sample_rate_hz))
            / u128::from(input.sample_rate_hz())) as usize;
        let mut output = vec![0.0; output_frames.saturating_mul(channels)];
        let ratio = f64::from(input.sample_rate_hz()) / f64::from(output_sample_rate_hz);
        for output_frame in 0..output_frames {
            let position = output_frame as f64 * ratio;
            let center = position.floor() as isize;
            let fraction = (position - center as f64) as f32;
            for channel in 0..channels {
                let sample = |frame: isize| {
                    let frame = frame.clamp(0, input_frames.saturating_sub(1) as isize) as usize;
                    input.samples()[frame * channels + channel]
                };
                let p0 = sample(center - 1);
                let p1 = sample(center);
                let p2 = sample(center + 1);
                let p3 = sample(center + 2);
                let a = -0.5 * p0 + 1.5 * p1 - 1.5 * p2 + 0.5 * p3;
                let b = p0 - 2.5 * p1 + 2.0 * p2 - 0.5 * p3;
                let c = -0.5 * p0 + 0.5 * p2;
                output[output_frame * channels + channel] =
                    ((a * fraction + b) * fraction + c) * fraction + p1;
            }
        }
        DecodedAudio::new(output_sample_rate_hz, input.channels(), output)
            .map_err(|error| AudioDecodeError::InvalidPcm(error.to_string()))
    }
}

pub trait AudioChunkDecoder {
    fn sample_rate_hz(&self) -> u32;
    fn channels(&self) -> u16;
    fn decode_next(&mut self, interleaved: &mut Vec<f32>) -> Result<ChunkState, AudioDecodeError>;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ChunkState {
    More,
    End,
}

#[derive(Debug, Error)]
pub enum AudioDecodeError {
    #[error("audio input limit exceeded: {0}")]
    Limit(String),
    #[error("failed to probe audio container: {0}")]
    Probe(String),
    #[error("audio file has no default track")]
    MissingTrack,
    #[error("audio track has no supported codec")]
    UnsupportedCodec,
    #[error("failed to read audio packet: {0}")]
    Packet(String),
    #[error("failed to create or run audio decoder: {0}")]
    Decoder(String),
    #[error("audio decoder requested a stream reset")]
    ResetRequired,
    #[error("audio stream changed: {0}")]
    StreamChanged(String),
    #[error("audio contains {0} channels; Arcweft bundle assets currently require mono or stereo")]
    UnsupportedChannels(usize),
    #[error("audio file decoded to no samples")]
    Empty,
    #[error("invalid decoded PCM: {0}")]
    InvalidPcm(String),
}

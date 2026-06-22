use arcweft_interaction_model::audio::MicrophoneConstraints;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, I24, Sample, SampleFormat, SizedSample, Stream, StreamConfig, U24};
use rtrb::{Consumer, Producer, PushError, RingBuffer};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use thiserror::Error;

pub struct NativeMicrophone {
    stream: Stream,
    samples: Consumer<f32>,
    dropped_samples: Arc<AtomicU64>,
    sample_rate_hz: u32,
    channels: u16,
}

impl NativeMicrophone {
    pub fn open_default(
        constraints: MicrophoneConstraints,
        sample_capacity: usize,
    ) -> Result<Self, NativeMicrophoneError> {
        if sample_capacity == 0 {
            return Err(NativeMicrophoneError::Configuration(
                "sample ring capacity must be non-zero".to_owned(),
            ));
        }
        let host = cpal::default_host();
        let device = host
            .default_input_device()
            .ok_or(NativeMicrophoneError::MissingInputDevice)?;
        let supported = device
            .default_input_config()
            .map_err(|error| NativeMicrophoneError::DefaultConfig(error.to_string()))?;
        let sample_format = supported.sample_format();
        let mut config: StreamConfig = supported.into();
        if constraints.channels > 0 && constraints.channels <= config.channels {
            config.channels = constraints.channels;
        }
        let sample_rate_hz = config.sample_rate;
        let channels = config.channels;
        let (producer, consumer) = RingBuffer::new(sample_capacity);
        let dropped_samples = Arc::new(AtomicU64::new(0));
        let stream = match sample_format {
            SampleFormat::I8 => {
                build_input::<i8>(&device, config, producer, Arc::clone(&dropped_samples))
            }
            SampleFormat::I16 => {
                build_input::<i16>(&device, config, producer, Arc::clone(&dropped_samples))
            }
            SampleFormat::I24 => {
                build_input::<I24>(&device, config, producer, Arc::clone(&dropped_samples))
            }
            SampleFormat::I32 => {
                build_input::<i32>(&device, config, producer, Arc::clone(&dropped_samples))
            }
            SampleFormat::I64 => {
                build_input::<i64>(&device, config, producer, Arc::clone(&dropped_samples))
            }
            SampleFormat::U8 => {
                build_input::<u8>(&device, config, producer, Arc::clone(&dropped_samples))
            }
            SampleFormat::U16 => {
                build_input::<u16>(&device, config, producer, Arc::clone(&dropped_samples))
            }
            SampleFormat::U24 => {
                build_input::<U24>(&device, config, producer, Arc::clone(&dropped_samples))
            }
            SampleFormat::U32 => {
                build_input::<u32>(&device, config, producer, Arc::clone(&dropped_samples))
            }
            SampleFormat::U64 => {
                build_input::<u64>(&device, config, producer, Arc::clone(&dropped_samples))
            }
            SampleFormat::F32 => {
                build_input::<f32>(&device, config, producer, Arc::clone(&dropped_samples))
            }
            SampleFormat::F64 => {
                build_input::<f64>(&device, config, producer, Arc::clone(&dropped_samples))
            }
            other => return Err(NativeMicrophoneError::UnsupportedSampleFormat(other)),
        }?;
        stream
            .play()
            .map_err(|error| NativeMicrophoneError::Play(error.to_string()))?;
        Ok(Self {
            stream,
            samples: consumer,
            dropped_samples,
            sample_rate_hz,
            channels,
        })
    }

    pub fn drain_samples(&mut self, output: &mut Vec<f32>, maximum: usize) {
        output.extend((0..maximum).map_while(|_| self.samples.pop().ok()));
    }

    pub fn dropped_sample_count(&self) -> u64 {
        self.dropped_samples.load(Ordering::Relaxed)
    }

    pub const fn sample_rate_hz(&self) -> u32 {
        self.sample_rate_hz
    }

    pub const fn channels(&self) -> u16 {
        self.channels
    }

    pub fn stream(&self) -> &Stream {
        &self.stream
    }
}

fn build_input<T>(
    device: &cpal::Device,
    config: StreamConfig,
    mut samples: Producer<f32>,
    dropped_samples: Arc<AtomicU64>,
) -> Result<Stream, NativeMicrophoneError>
where
    T: SizedSample,
    f32: FromSample<T>,
{
    device
        .build_input_stream(
            config,
            move |input: &[T], _| {
                for sample in input.iter().copied().map(f32::from_sample) {
                    if let Err(PushError::Full(_)) = samples.push(sample) {
                        dropped_samples.fetch_add(1, Ordering::Relaxed);
                    }
                }
            },
            move |_error| {},
            None,
        )
        .map_err(|error| NativeMicrophoneError::BuildStream(error.to_string()))
}

#[derive(Debug, Error)]
pub enum NativeMicrophoneError {
    #[error("invalid microphone configuration: {0}")]
    Configuration(String),
    #[error("no default input device is available")]
    MissingInputDevice,
    #[error("failed to read the default input config: {0}")]
    DefaultConfig(String),
    #[error("unsupported input sample format {0}")]
    UnsupportedSampleFormat(SampleFormat),
    #[error("failed to build the CPAL input stream: {0}")]
    BuildStream(String),
    #[error("failed to start the CPAL input stream: {0}")]
    Play(String),
}

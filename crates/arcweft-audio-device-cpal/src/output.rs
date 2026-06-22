use arcweft_audio_core::{PreparedAudioCommand, PreparedAudioGraph};
use arcweft_audio_mixer::Mixer;
use arcweft_interaction_model::audio::{AudioEvent, AudioFailure};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, I24, SampleFormat, SizedSample, Stream, StreamConfig, U24};
use rtrb::{Consumer, Producer, PushError, RingBuffer};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use thiserror::Error;

#[derive(Clone, Copy, Debug)]
pub struct CpalOutputConfig {
    pub command_capacity: usize,
    pub event_capacity: usize,
    pub max_callback_frames: usize,
}

impl Default for CpalOutputConfig {
    fn default() -> Self {
        Self {
            command_capacity: 2_048,
            event_capacity: 2_048,
            max_callback_frames: 8_192,
        }
    }
}

pub struct CpalOutput {
    stream: Stream,
    commands: Producer<PreparedAudioCommand>,
    events: Consumer<AudioEvent>,
    dropped_events: Arc<AtomicU64>,
    sample_rate_hz: u32,
    channels: u16,
}

impl CpalOutput {
    pub fn open_default(
        graph: PreparedAudioGraph,
        options: CpalOutputConfig,
    ) -> Result<Self, CpalOutputError> {
        if options.command_capacity == 0
            || options.event_capacity == 0
            || options.max_callback_frames == 0
        {
            return Err(CpalOutputError::Configuration(
                "ring and callback capacities must be non-zero".to_owned(),
            ));
        }
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .ok_or(CpalOutputError::MissingOutputDevice)?;
        let supported = device
            .default_output_config()
            .map_err(|error| CpalOutputError::DefaultConfig(error.to_string()))?;
        let sample_format = supported.sample_format();
        let stream_config: StreamConfig = supported.into();
        let sample_rate_hz = stream_config.sample_rate;
        let channels = stream_config.channels;
        let (command_tx, command_rx) = RingBuffer::new(options.command_capacity);
        let (event_tx, event_rx) = RingBuffer::new(options.event_capacity);
        let dropped_events = Arc::new(AtomicU64::new(0));

        macro_rules! build_default_output {
            ($sample:ty) => {
                build_output::<$sample>(
                    &device,
                    stream_config,
                    graph,
                    command_rx,
                    event_tx,
                    &dropped_events,
                    options.max_callback_frames,
                )
            };
        }

        let stream = match sample_format {
            SampleFormat::I8 => build_default_output!(i8),
            SampleFormat::I16 => build_default_output!(i16),
            SampleFormat::I24 => build_default_output!(I24),
            SampleFormat::I32 => build_default_output!(i32),
            SampleFormat::I64 => build_default_output!(i64),
            SampleFormat::U8 => build_default_output!(u8),
            SampleFormat::U16 => build_default_output!(u16),
            SampleFormat::U24 => build_default_output!(U24),
            SampleFormat::U32 => build_default_output!(u32),
            SampleFormat::U64 => build_default_output!(u64),
            SampleFormat::F32 => build_default_output!(f32),
            SampleFormat::F64 => build_default_output!(f64),
            other => return Err(CpalOutputError::UnsupportedSampleFormat(other)),
        }?;
        stream
            .play()
            .map_err(|error| CpalOutputError::Play(error.to_string()))?;

        Ok(Self {
            stream,
            commands: command_tx,
            events: event_rx,
            dropped_events,
            sample_rate_hz,
            channels,
        })
    }

    pub fn submit(&mut self, command: PreparedAudioCommand) -> Result<(), CpalOutputError> {
        self.commands
            .push(command)
            .map_err(|PushError::Full(_)| CpalOutputError::CommandQueueFull)
    }

    pub fn drain_events(&mut self, output: &mut Vec<AudioEvent>) {
        while let Ok(event) = self.events.pop() {
            output.push(event);
        }
    }

    pub fn dropped_event_count(&self) -> u64 {
        self.dropped_events.load(Ordering::Relaxed)
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

fn build_output<T>(
    device: &cpal::Device,
    config: StreamConfig,
    graph: PreparedAudioGraph,
    mut commands: Consumer<PreparedAudioCommand>,
    mut events: Producer<AudioEvent>,
    dropped_events: &Arc<AtomicU64>,
    max_callback_frames: usize,
) -> Result<Stream, CpalOutputError>
where
    T: SizedSample + FromSample<f32>,
{
    let device_channels = usize::from(config.channels);
    let sample_rate_hz = config.sample_rate;
    let mut mixer = Mixer::new(graph, sample_rate_hz, max_callback_frames)
        .map_err(|error| CpalOutputError::Mixer(error.to_string()))?;
    let mut stereo = vec![0.0; max_callback_frames * 2];
    let callback_drops = Arc::clone(dropped_events);
    let stream = device
        .build_output_stream(
            config,
            move |device_output: &mut [T], _| {
                while let Ok(command) = commands.pop() {
                    let dispatch = command.dispatch();
                    if let Err(error) = mixer.apply(command, |event| {
                        if events.push(event).is_err() {
                            callback_drops.fetch_add(1, Ordering::Relaxed);
                        }
                    }) && let Some(dispatch) = dispatch
                        && events
                            .push(AudioEvent::CommandFailed {
                                dispatch,
                                failure: AudioFailure::Backend {
                                    message: error.to_string(),
                                },
                            })
                            .is_err()
                    {
                        callback_drops.fetch_add(1, Ordering::Relaxed);
                    }
                }
                let frames = device_output.len() / device_channels;
                let stereo_samples = frames.saturating_mul(2);
                if stereo_samples > stereo.len() || device_channels == 0 {
                    device_output.fill(T::from_sample(0.0));
                    return;
                }
                mixer.render(&mut stereo[..stereo_samples], |event| {
                    if events.push(event).is_err() {
                        callback_drops.fetch_add(1, Ordering::Relaxed);
                    }
                });
                for (frame_index, frame) in
                    device_output.chunks_exact_mut(device_channels).enumerate()
                {
                    let left = stereo[frame_index * 2];
                    let right = stereo[frame_index * 2 + 1];
                    if device_channels == 1 {
                        frame[0] = T::from_sample((left + right) * 0.5);
                    } else {
                        frame[0] = T::from_sample(left);
                        frame[1] = T::from_sample(right);
                        for sample in &mut frame[2..] {
                            *sample = T::from_sample(0.0);
                        }
                    }
                }
            },
            move |_error| {},
            None,
        )
        .map_err(|error| CpalOutputError::BuildStream(error.to_string()))?;
    Ok(stream)
}

#[derive(Debug, Error)]
pub enum CpalOutputError {
    #[error("invalid CPAL output configuration: {0}")]
    Configuration(String),
    #[error("no default output device is available")]
    MissingOutputDevice,
    #[error("failed to read the default output config: {0}")]
    DefaultConfig(String),
    #[error("unsupported output sample format {0}")]
    UnsupportedSampleFormat(SampleFormat),
    #[error("failed to create the Arcweft mixer: {0}")]
    Mixer(String),
    #[error("failed to build the CPAL output stream: {0}")]
    BuildStream(String),
    #[error("failed to start the CPAL output stream: {0}")]
    Play(String),
    #[error("the bounded audio command queue is full")]
    CommandQueueFull,
}

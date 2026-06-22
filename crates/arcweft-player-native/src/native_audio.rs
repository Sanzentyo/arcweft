use arcweft_audio_codec::{AudioDecodeLimits, AudioResampler, CubicResampler, decode_audio};
use arcweft_audio_core::{AudioCommandPreparer, AudioDispatch, DEFAULT_MAX_VOICES};
use arcweft_audio_device_cpal::{
    CpalOutput, CpalOutputConfig, CpalOutputError, NativeMicrophone, NativeMicrophoneError,
};
use arcweft_bundle::{ArcweftBundle, BundleCodecError};
use arcweft_interaction_model::audio::{
    AudioCaptureId, AudioCaptureState, AudioCommand, AudioCommandEnvelope, AudioEvent, AudioFailure,
};
use std::collections::BTreeMap;
use std::sync::Arc;
use thiserror::Error;

const MICROPHONE_SAMPLE_CAPACITY: usize = 48_000 * 4;
const MICROPHONE_DRAIN_LIMIT: usize = 4_096;

pub(super) struct NativeAudioRuntime {
    output: CpalOutput,
    preparer: AudioCommandPreparer,
    microphones: BTreeMap<AudioCaptureId, NativeMicrophone>,
    microphone_sequences: BTreeMap<AudioCaptureId, u64>,
    microphone_samples: Vec<f32>,
}

impl NativeAudioRuntime {
    pub(super) fn from_bundle(
        bundle: &ArcweftBundle,
    ) -> Result<Option<Self>, NativePlayerAudioError> {
        let Some(graph) = bundle.audio.as_ref() else {
            return Ok(None);
        };
        let (prepared_graph, preparer) = graph.prepare(DEFAULT_MAX_VOICES)?;
        let mut output = CpalOutput::open_default(prepared_graph, CpalOutputConfig::default())?;
        let resampler = CubicResampler;
        for asset in &graph.assets {
            let bytes = bundle
                .audio_asset_bytes(asset.id.as_str())?
                .ok_or_else(|| NativePlayerAudioError::MissingDeclaredAsset {
                    asset: asset.id.as_str().to_owned(),
                })?;
            let decoded = decode_audio(bytes, asset.format, AudioDecodeLimits::default())?;
            let decoded = if decoded.sample_rate_hz() == output.sample_rate_hz() {
                decoded
            } else {
                resampler.resample(&decoded, output.sample_rate_hz())?
            };
            output.submit(
                preparer
                    .install_resource(&asset.id, Arc::new(decoded))
                    .map_err(NativePlayerAudioError::InstallResource)?,
            )?;
        }
        Ok(Some(Self {
            output,
            preparer,
            microphones: BTreeMap::new(),
            microphone_sequences: BTreeMap::new(),
            microphone_samples: Vec::new(),
        }))
    }

    pub(super) fn drain_events(&mut self, output: &mut Vec<AudioEvent>) {
        let start = output.len();
        self.output.drain_events(output);
        for event in &output[start..] {
            self.preparer.observe_event(event);
        }
        for (capture, microphone) in &mut self.microphones {
            self.microphone_samples.clear();
            microphone.drain_samples(&mut self.microphone_samples, MICROPHONE_DRAIN_LIMIT);
            if self.microphone_samples.is_empty() {
                continue;
            }
            let peak = self
                .microphone_samples
                .iter()
                .fold(0.0_f32, |peak, sample| peak.max(sample.abs()));
            let square_sum = self
                .microphone_samples
                .iter()
                .map(|sample| sample * sample)
                .sum::<f32>();
            let sample_count = u16::try_from(self.microphone_samples.len())
                .expect("microphone drain limit fits in u16");
            let rms = (square_sum / f32::from(sample_count)).sqrt();
            let sequence = self
                .microphone_sequences
                .entry(capture.clone())
                .and_modify(|sequence| *sequence = sequence.saturating_add(1))
                .or_insert(0);
            output.push(AudioEvent::CaptureLevel {
                capture: capture.clone(),
                sequence: *sequence,
                rms,
                peak,
                dropped_samples: microphone.dropped_sample_count(),
            });
        }
    }

    pub(super) fn submit_commands(
        &mut self,
        commands: Vec<AudioCommandEnvelope>,
        events: &mut Vec<AudioEvent>,
    ) {
        for envelope in commands {
            let dispatch = envelope.dispatch;
            match envelope.command {
                AudioCommand::RequestMicrophone {
                    capture,
                    constraints,
                } => {
                    match NativeMicrophone::open_default(constraints, MICROPHONE_SAMPLE_CAPACITY) {
                        Ok(microphone) => {
                            let sample_rate_hz = microphone.sample_rate_hz();
                            let channels = microphone.channels();
                            self.microphones.insert(capture.clone(), microphone);
                            self.microphone_sequences.insert(capture.clone(), 0);
                            events.push(AudioEvent::CaptureStateChanged {
                                dispatch,
                                capture,
                                state: AudioCaptureState::Started,
                                sample_rate_hz: Some(sample_rate_hz),
                                channels: Some(channels),
                            });
                        }
                        Err(error) => events.push(AudioEvent::CommandFailed {
                            dispatch,
                            failure: native_microphone_failure(capture, error),
                        }),
                    }
                }
                AudioCommand::StopMicrophone { capture } => {
                    if self.microphones.remove(&capture).is_some() {
                        self.microphone_sequences.remove(&capture);
                        events.push(AudioEvent::CaptureStateChanged {
                            dispatch,
                            capture,
                            state: AudioCaptureState::Stopped,
                            sample_rate_hz: None,
                            channels: None,
                        });
                    } else {
                        events.push(AudioEvent::CommandFailed {
                            dispatch,
                            failure: AudioFailure::UnknownCapture { capture },
                        });
                    }
                }
                AudioCommand::SetCaptureMonitor { capture, .. } => {
                    events.push(AudioEvent::CommandFailed {
                        dispatch,
                        failure: AudioFailure::Backend {
                            message: format!(
                                "native CPAL capture monitor routing is not implemented for `{}`",
                                capture.as_str()
                            ),
                        },
                    });
                }
                command => {
                    match self.preparer.prepare(AudioDispatch {
                        id: dispatch,
                        command,
                    }) {
                        Ok(command) => {
                            if let Err(error) = self.output.submit(command) {
                                events.push(AudioEvent::CommandFailed {
                                    dispatch,
                                    failure: cpal_output_failure(error),
                                });
                            }
                        }
                        Err(failure) => {
                            events.push(AudioEvent::CommandFailed { dispatch, failure });
                        }
                    }
                }
            }
        }
    }
}

fn cpal_output_failure(error: CpalOutputError) -> AudioFailure {
    match error {
        CpalOutputError::CommandQueueFull => AudioFailure::QueueFull,
        error => AudioFailure::Backend {
            message: error.to_string(),
        },
    }
}

fn native_microphone_failure(
    capture: AudioCaptureId,
    error: NativeMicrophoneError,
) -> AudioFailure {
    match error {
        NativeMicrophoneError::MissingInputDevice => AudioFailure::PermissionDenied { capture },
        error => AudioFailure::Backend {
            message: error.to_string(),
        },
    }
}

#[derive(Debug, Error)]
pub enum NativePlayerAudioError {
    #[error(transparent)]
    Graph(#[from] arcweft_audio_core::AudioGraphError),
    #[error(transparent)]
    Decode(#[from] arcweft_audio_codec::AudioDecodeError),
    #[error(transparent)]
    Bundle(#[from] BundleCodecError),
    #[error(transparent)]
    Output(#[from] CpalOutputError),
    #[error("audio graph declared asset `{asset}` but the bundle did not expose it")]
    MissingDeclaredAsset { asset: String },
    #[error("failed to prepare audio resource install command: {0:?}")]
    InstallResource(AudioFailure),
}

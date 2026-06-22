use arcweft_audio_codec::{AudioDecodeLimits, AudioResampler, CubicResampler, decode_audio};
use arcweft_audio_core::{AudioCommandPreparer, AudioDispatch, DEFAULT_MAX_VOICES};
use arcweft_audio_device_cpal::{CpalOutput, CpalOutputConfig, CpalOutputError};
use arcweft_bundle::{ArcweftBundle, BundleCodecError};
use arcweft_interaction_model::audio::{AudioCommandEnvelope, AudioEvent, AudioFailure};
use std::sync::Arc;
use thiserror::Error;

pub(super) struct NativeAudioRuntime {
    output: CpalOutput,
    preparer: AudioCommandPreparer,
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
        Ok(Some(Self { output, preparer }))
    }

    pub(super) fn drain_events(&mut self, output: &mut Vec<AudioEvent>) {
        let start = output.len();
        self.output.drain_events(output);
        for event in &output[start..] {
            self.preparer.observe_event(event);
        }
    }

    pub(super) fn submit_commands(
        &mut self,
        commands: Vec<AudioCommandEnvelope>,
        events: &mut Vec<AudioEvent>,
    ) {
        for envelope in commands {
            let dispatch = envelope.dispatch;
            match self.preparer.prepare(AudioDispatch {
                id: dispatch,
                command: envelope.command,
            }) {
                Ok(command) => {
                    if let Err(error) = self.output.submit(command) {
                        events.push(AudioEvent::CommandFailed {
                            dispatch,
                            failure: cpal_output_failure(error),
                        });
                    }
                }
                Err(failure) => events.push(AudioEvent::CommandFailed { dispatch, failure }),
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

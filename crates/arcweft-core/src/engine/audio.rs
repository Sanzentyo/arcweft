use arcweft_interaction_model::audio::{
    AudioBusId, AudioCaptureId, AudioCommand, AudioCommandEnvelope, AudioDispatchId, AudioEffectId,
    AudioMillis, AudioResourceId, AudioSnapshotId, AudioVoiceId, GainDbMilli, PanMilli,
};

use super::{
    Engine, FlowFiberStatus, LineEffectRequest, RuntimeDiagnostic, RuntimeEvalError,
    RuntimeStepOutput,
};
use crate::audio::RuntimeAudioCommand;
use crate::effect::RuntimeEffectExpr;
use crate::pure::RuntimeCallBackend;
use crate::value::{RuntimeExpr, RuntimeValue, runtime_value_label};

impl Engine {
    pub(super) fn evaluate_effect_expr(
        &mut self,
        effect: &RuntimeEffectExpr,
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> Result<Option<LineEffectRequest>, RuntimeEvalError> {
        let values = effect
            .argument_exprs()
            .into_iter()
            .map(|expr| self.evaluate_expr_with_backend(expr, pure_backend))
            .collect::<Result<Vec<_>, _>>()?;
        effect
            .materialize(&values)
            .map_err(|error| RuntimeEvalError::Effect(error.to_string()))
    }

    /// Emits one lowered effect while resolving audio at the owning fiber's
    /// evaluation point. This keeps child-fiber bindings available and prevents
    /// host adapters from re-parsing string call labels.
    pub(super) fn emit_line_effect(
        &mut self,
        effect: LineEffectRequest,
        output: &mut RuntimeStepOutput,
        pure_backend: &mut impl RuntimeCallBackend,
    ) {
        let LineEffectRequest::Audio(command) = effect else {
            output.effects.line.push(effect);
            return;
        };
        match self.evaluate_audio_command(&command, pure_backend) {
            Ok(command) => output.requests.audio.push(AudioCommandEnvelope::new(
                self.next_audio_dispatch(),
                command,
            )),
            Err(error) => {
                let message = error.to_string();
                output
                    .diagnostics
                    .push(RuntimeDiagnostic::new(message.clone()));
                self.fiber.status = FlowFiberStatus::Failed(message);
            }
        }
    }

    pub(super) fn emit_line_effects(
        &mut self,
        effects: impl IntoIterator<Item = LineEffectRequest>,
        output: &mut RuntimeStepOutput,
        pure_backend: &mut impl RuntimeCallBackend,
    ) {
        for effect in effects {
            self.emit_line_effect(effect, output, pure_backend);
            if matches!(self.fiber.status, FlowFiberStatus::Failed(_)) {
                break;
            }
        }
    }

    pub(super) fn merge_step_output(
        &mut self,
        mut other: RuntimeStepOutput,
        output: &mut RuntimeStepOutput,
        pure_backend: &mut impl RuntimeCallBackend,
    ) {
        let effects = std::mem::take(&mut other.effects.line);
        output.merge(other);
        self.emit_line_effects(effects, output, pure_backend);
    }

    fn next_audio_dispatch(&mut self) -> AudioDispatchId {
        let dispatch = AudioDispatchId::new(self.audio_epoch, self.next_audio_sequence);
        self.next_audio_sequence = self.next_audio_sequence.saturating_add(1);
        dispatch
    }

    #[allow(clippy::too_many_lines)]
    fn evaluate_audio_command(
        &mut self,
        command: &RuntimeAudioCommand,
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> Result<AudioCommand, RuntimeEvalError> {
        match command {
            RuntimeAudioCommand::Play {
                voice,
                resource,
                bus,
                gain_db_milli,
                pan_milli,
                loop_mode,
                start_frame,
                fade_in_millis,
            } => Ok(AudioCommand::Play {
                voice: AudioVoiceId::new(self.evaluate_audio_identifier(voice, pure_backend)?)
                    .map_err(|error| RuntimeEvalError::Audio(error.to_string()))?,
                resource: AudioResourceId::new(
                    self.evaluate_audio_identifier(resource, pure_backend)?,
                )
                .map_err(|error| RuntimeEvalError::Audio(error.to_string()))?,
                bus: AudioBusId::new(self.evaluate_audio_identifier(bus, pure_backend)?)
                    .map_err(|error| RuntimeEvalError::Audio(error.to_string()))?,
                gain: GainDbMilli::new(self.evaluate_audio_i32(gain_db_milli, pure_backend)?)
                    .map_err(|error| RuntimeEvalError::Audio(error.to_string()))?,
                pan: PanMilli::new(self.evaluate_audio_i16(pan_milli, pure_backend)?)
                    .map_err(|error| RuntimeEvalError::Audio(error.to_string()))?,
                loop_mode: *loop_mode,
                start_frame: self.evaluate_audio_u64(start_frame, pure_backend)?,
                fade_in: AudioMillis::new(self.evaluate_audio_u32(fade_in_millis, pure_backend)?),
            }),
            RuntimeAudioCommand::Stop {
                voice,
                fade_out_millis,
            } => Ok(AudioCommand::Stop {
                voice: AudioVoiceId::new(self.evaluate_audio_identifier(voice, pure_backend)?)
                    .map_err(|error| RuntimeEvalError::Audio(error.to_string()))?,
                fade_out: AudioMillis::new(self.evaluate_audio_u32(fade_out_millis, pure_backend)?),
            }),
            RuntimeAudioCommand::StopAll { fade_out_millis } => Ok(AudioCommand::StopAll {
                fade_out: AudioMillis::new(self.evaluate_audio_u32(fade_out_millis, pure_backend)?),
            }),
            RuntimeAudioCommand::SetVoiceGain {
                voice,
                gain_db_milli,
                transition_millis,
            } => Ok(AudioCommand::SetVoiceGain {
                voice: AudioVoiceId::new(self.evaluate_audio_identifier(voice, pure_backend)?)
                    .map_err(|error| RuntimeEvalError::Audio(error.to_string()))?,
                gain: GainDbMilli::new(self.evaluate_audio_i32(gain_db_milli, pure_backend)?)
                    .map_err(|error| RuntimeEvalError::Audio(error.to_string()))?,
                transition: AudioMillis::new(
                    self.evaluate_audio_u32(transition_millis, pure_backend)?,
                ),
            }),
            RuntimeAudioCommand::SetVoicePan {
                voice,
                pan_milli,
                transition_millis,
            } => Ok(AudioCommand::SetVoicePan {
                voice: AudioVoiceId::new(self.evaluate_audio_identifier(voice, pure_backend)?)
                    .map_err(|error| RuntimeEvalError::Audio(error.to_string()))?,
                pan: PanMilli::new(self.evaluate_audio_i16(pan_milli, pure_backend)?)
                    .map_err(|error| RuntimeEvalError::Audio(error.to_string()))?,
                transition: AudioMillis::new(
                    self.evaluate_audio_u32(transition_millis, pure_backend)?,
                ),
            }),
            RuntimeAudioCommand::SetBusGain {
                bus,
                gain_db_milli,
                transition_millis,
            } => Ok(AudioCommand::SetBusGain {
                bus: AudioBusId::new(self.evaluate_audio_identifier(bus, pure_backend)?)
                    .map_err(|error| RuntimeEvalError::Audio(error.to_string()))?,
                gain: GainDbMilli::new(self.evaluate_audio_i32(gain_db_milli, pure_backend)?)
                    .map_err(|error| RuntimeEvalError::Audio(error.to_string()))?,
                transition: AudioMillis::new(
                    self.evaluate_audio_u32(transition_millis, pure_backend)?,
                ),
            }),
            RuntimeAudioCommand::SetBusMute { bus, muted } => Ok(AudioCommand::SetBusMute {
                bus: AudioBusId::new(self.evaluate_audio_identifier(bus, pure_backend)?)
                    .map_err(|error| RuntimeEvalError::Audio(error.to_string()))?,
                muted: self.evaluate_audio_bool(muted, pure_backend)?,
            }),
            RuntimeAudioCommand::SetEffectEnabled {
                bus,
                effect,
                enabled,
            } => Ok(AudioCommand::SetEffectEnabled {
                bus: AudioBusId::new(self.evaluate_audio_identifier(bus, pure_backend)?)
                    .map_err(|error| RuntimeEvalError::Audio(error.to_string()))?,
                effect: AudioEffectId::new(self.evaluate_audio_identifier(effect, pure_backend)?)
                    .map_err(|error| RuntimeEvalError::Audio(error.to_string()))?,
                enabled: self.evaluate_audio_bool(enabled, pure_backend)?,
            }),
            RuntimeAudioCommand::SetEffectParameter {
                bus,
                effect,
                parameter,
                value,
                transition_millis,
            } => Ok(AudioCommand::SetEffectParameter {
                bus: AudioBusId::new(self.evaluate_audio_identifier(bus, pure_backend)?)
                    .map_err(|error| RuntimeEvalError::Audio(error.to_string()))?,
                effect: AudioEffectId::new(self.evaluate_audio_identifier(effect, pure_backend)?)
                    .map_err(|error| RuntimeEvalError::Audio(error.to_string()))?,
                parameter: parameter
                    .from_i64(self.evaluate_audio_i64(value, pure_backend)?)
                    .map_err(|error| RuntimeEvalError::Audio(error.to_string()))?,
                transition: AudioMillis::new(
                    self.evaluate_audio_u32(transition_millis, pure_backend)?,
                ),
            }),
            RuntimeAudioCommand::ApplySnapshot {
                snapshot,
                transition_millis,
            } => Ok(AudioCommand::ApplySnapshot {
                snapshot: AudioSnapshotId::new(
                    self.evaluate_audio_identifier(snapshot, pure_backend)?,
                )
                .map_err(|error| RuntimeEvalError::Audio(error.to_string()))?,
                transition: AudioMillis::new(
                    self.evaluate_audio_u32(transition_millis, pure_backend)?,
                ),
            }),
            RuntimeAudioCommand::RequestMicrophone {
                capture,
                constraints,
            } => Ok(AudioCommand::RequestMicrophone {
                capture: AudioCaptureId::new(
                    self.evaluate_audio_identifier(capture, pure_backend)?,
                )
                .map_err(|error| RuntimeEvalError::Audio(error.to_string()))?,
                constraints: constraints
                    .validate()
                    .map_err(|error| RuntimeEvalError::Audio(error.to_string()))?,
            }),
            RuntimeAudioCommand::StopMicrophone { capture } => Ok(AudioCommand::StopMicrophone {
                capture: AudioCaptureId::new(
                    self.evaluate_audio_identifier(capture, pure_backend)?,
                )
                .map_err(|error| RuntimeEvalError::Audio(error.to_string()))?,
            }),
            RuntimeAudioCommand::SetCaptureMonitor {
                capture,
                bus,
                gain_db_milli,
            } => {
                let bus = match bus {
                    Some(bus) => Some(
                        AudioBusId::new(self.evaluate_audio_identifier(bus, pure_backend)?)
                            .map_err(|error| RuntimeEvalError::Audio(error.to_string()))?,
                    ),
                    None => None,
                };
                Ok(AudioCommand::SetCaptureMonitor {
                    capture: AudioCaptureId::new(
                        self.evaluate_audio_identifier(capture, pure_backend)?,
                    )
                    .map_err(|error| RuntimeEvalError::Audio(error.to_string()))?,
                    bus,
                    gain: GainDbMilli::new(self.evaluate_audio_i32(gain_db_milli, pure_backend)?)
                        .map_err(|error| RuntimeEvalError::Audio(error.to_string()))?,
                })
            }
        }
    }

    fn evaluate_audio_value(
        &mut self,
        expression: &RuntimeExpr,
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        self.evaluate_expr_with_backend(expression, pure_backend)
    }

    fn evaluate_audio_identifier(
        &mut self,
        expression: &RuntimeExpr,
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> Result<String, RuntimeEvalError> {
        let value = self.evaluate_audio_value(expression, pure_backend)?;
        value
            .as_identifier()
            .map(str::to_owned)
            .ok_or_else(|| RuntimeEvalError::AudioValue {
                expected: "audio identifier",
                actual: runtime_value_label(&value),
            })
    }

    fn evaluate_audio_bool(
        &mut self,
        expression: &RuntimeExpr,
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> Result<bool, RuntimeEvalError> {
        let value = self.evaluate_audio_value(expression, pure_backend)?;
        value.as_bool().ok_or_else(|| RuntimeEvalError::AudioValue {
            expected: "bool",
            actual: runtime_value_label(&value),
        })
    }

    fn evaluate_audio_i64(
        &mut self,
        expression: &RuntimeExpr,
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> Result<i64, RuntimeEvalError> {
        let value = self.evaluate_audio_value(expression, pure_backend)?;
        value.try_i64().ok_or_else(|| RuntimeEvalError::AudioValue {
            expected: "i64-compatible integer",
            actual: runtime_value_label(&value),
        })
    }

    fn evaluate_audio_i16(
        &mut self,
        expression: &RuntimeExpr,
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> Result<i16, RuntimeEvalError> {
        let value = self.evaluate_audio_value(expression, pure_backend)?;
        value
            .try_i64()
            .and_then(|value| i16::try_from(value).ok())
            .ok_or_else(|| RuntimeEvalError::AudioValue {
                expected: "i16",
                actual: runtime_value_label(&value),
            })
    }

    fn evaluate_audio_i32(
        &mut self,
        expression: &RuntimeExpr,
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> Result<i32, RuntimeEvalError> {
        let value = self.evaluate_audio_value(expression, pure_backend)?;
        value
            .try_i64()
            .and_then(|value| i32::try_from(value).ok())
            .ok_or_else(|| RuntimeEvalError::AudioValue {
                expected: "i32",
                actual: runtime_value_label(&value),
            })
    }

    fn evaluate_audio_u32(
        &mut self,
        expression: &RuntimeExpr,
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> Result<u32, RuntimeEvalError> {
        let value = self.evaluate_audio_value(expression, pure_backend)?;
        value
            .try_u64()
            .and_then(|value| u32::try_from(value).ok())
            .ok_or_else(|| RuntimeEvalError::AudioValue {
                expected: "u32",
                actual: runtime_value_label(&value),
            })
    }

    fn evaluate_audio_u64(
        &mut self,
        expression: &RuntimeExpr,
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> Result<u64, RuntimeEvalError> {
        let value = self.evaluate_audio_value(expression, pure_backend)?;
        value.try_u64().ok_or_else(|| RuntimeEvalError::AudioValue {
            expected: "u64",
            actual: runtime_value_label(&value),
        })
    }
}

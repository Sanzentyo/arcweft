use crate::value::RuntimeExpr;
use arcweft_interaction_model::audio::{
    AudioEffectParameterKind, AudioLoopMode, MicrophoneConstraints,
};

/// Typed runtime IR for audio commands whose values are evaluated by `Engine`.
#[allow(clippy::large_enum_variant)]
#[derive(Clone, Debug, PartialEq)]
pub enum RuntimeAudioCommand<Expression = RuntimeExpr> {
    Play {
        voice: Expression,
        resource: Expression,
        bus: Expression,
        gain_db_milli: Expression,
        pan_milli: Expression,
        loop_mode: AudioLoopMode,
        start_frame: Expression,
        fade_in_millis: Expression,
    },
    Stop {
        voice: Expression,
        fade_out_millis: Expression,
    },
    StopAll {
        fade_out_millis: Expression,
    },
    SetVoiceGain {
        voice: Expression,
        gain_db_milli: Expression,
        transition_millis: Expression,
    },
    SetVoicePan {
        voice: Expression,
        pan_milli: Expression,
        transition_millis: Expression,
    },
    SetBusGain {
        bus: Expression,
        gain_db_milli: Expression,
        transition_millis: Expression,
    },
    SetBusMute {
        bus: Expression,
        muted: Expression,
    },
    SetEffectEnabled {
        bus: Expression,
        effect: Expression,
        enabled: Expression,
    },
    SetEffectParameter {
        bus: Expression,
        effect: Expression,
        parameter: AudioEffectParameterKind,
        value: Expression,
        transition_millis: Expression,
    },
    ApplySnapshot {
        snapshot: Expression,
        transition_millis: Expression,
    },
    RequestMicrophone {
        capture: Expression,
        constraints: MicrophoneConstraints,
    },
    StopMicrophone {
        capture: Expression,
    },
    SetCaptureMonitor {
        capture: Expression,
        bus: Option<Expression>,
        gain_db_milli: Expression,
    },
}

impl<Expression> RuntimeAudioCommand<Expression> {
    /// Structural metadata which can be checked before evaluating expressions
    /// or consulting decoded audio resources and devices.
    pub(crate) fn validate_metadata(
        &self,
    ) -> Result<(), arcweft_interaction_model::audio::AudioValueError> {
        match self {
            Self::Play {
                loop_mode:
                    AudioLoopMode::Region {
                        start_frame,
                        end_frame,
                    },
                ..
            } => AudioLoopMode::region(*start_frame, *end_frame).map(|_| ()),
            Self::RequestMicrophone { constraints, .. } => constraints.validate().map(|_| ()),
            _ => Ok(()),
        }
    }

    /// Borrows expression slots without cloning their graphs. Metadata remains
    /// the same typed data in the same closed operation grammar.
    pub(crate) fn expression_refs(&self) -> RuntimeAudioCommand<&Expression> {
        match self {
            Self::Play {
                voice,
                resource,
                bus,
                gain_db_milli,
                pan_milli,
                loop_mode,
                start_frame,
                fade_in_millis,
            } => RuntimeAudioCommand::Play {
                voice,
                resource,
                bus,
                gain_db_milli,
                pan_milli,
                loop_mode: *loop_mode,
                start_frame,
                fade_in_millis,
            },
            Self::Stop {
                voice,
                fade_out_millis,
            } => RuntimeAudioCommand::Stop {
                voice,
                fade_out_millis,
            },
            Self::StopAll { fade_out_millis } => RuntimeAudioCommand::StopAll { fade_out_millis },
            Self::SetVoiceGain {
                voice,
                gain_db_milli,
                transition_millis,
            } => RuntimeAudioCommand::SetVoiceGain {
                voice,
                gain_db_milli,
                transition_millis,
            },
            Self::SetVoicePan {
                voice,
                pan_milli,
                transition_millis,
            } => RuntimeAudioCommand::SetVoicePan {
                voice,
                pan_milli,
                transition_millis,
            },
            Self::SetBusGain {
                bus,
                gain_db_milli,
                transition_millis,
            } => RuntimeAudioCommand::SetBusGain {
                bus,
                gain_db_milli,
                transition_millis,
            },
            Self::SetBusMute { bus, muted } => RuntimeAudioCommand::SetBusMute { bus, muted },
            Self::SetEffectEnabled {
                bus,
                effect,
                enabled,
            } => RuntimeAudioCommand::SetEffectEnabled {
                bus,
                effect,
                enabled,
            },
            Self::SetEffectParameter {
                bus,
                effect,
                parameter,
                value,
                transition_millis,
            } => RuntimeAudioCommand::SetEffectParameter {
                bus,
                effect,
                parameter: *parameter,
                value,
                transition_millis,
            },
            Self::ApplySnapshot {
                snapshot,
                transition_millis,
            } => RuntimeAudioCommand::ApplySnapshot {
                snapshot,
                transition_millis,
            },
            Self::RequestMicrophone {
                capture,
                constraints,
            } => RuntimeAudioCommand::RequestMicrophone {
                capture,
                constraints: *constraints,
            },
            Self::StopMicrophone { capture } => RuntimeAudioCommand::StopMicrophone { capture },
            Self::SetCaptureMonitor {
                capture,
                bus,
                gain_db_milli,
            } => RuntimeAudioCommand::SetCaptureMonitor {
                capture,
                bus: bus.as_ref(),
                gain_db_milli,
            },
        }
    }

    /// Maps each expression exactly once in command ABI order, stopping before
    /// later slots if the owning context rejects a child.
    pub(crate) fn try_map_expressions<Mapped, Error>(
        self,
        mut map: impl FnMut(Expression) -> Result<Mapped, Error>,
    ) -> Result<RuntimeAudioCommand<Mapped>, Error> {
        Ok(match self {
            Self::Play {
                voice,
                resource,
                bus,
                gain_db_milli,
                pan_milli,
                loop_mode,
                start_frame,
                fade_in_millis,
            } => RuntimeAudioCommand::Play {
                voice: map(voice)?,
                resource: map(resource)?,
                bus: map(bus)?,
                gain_db_milli: map(gain_db_milli)?,
                pan_milli: map(pan_milli)?,
                loop_mode,
                start_frame: map(start_frame)?,
                fade_in_millis: map(fade_in_millis)?,
            },
            Self::Stop {
                voice,
                fade_out_millis,
            } => RuntimeAudioCommand::Stop {
                voice: map(voice)?,
                fade_out_millis: map(fade_out_millis)?,
            },
            Self::StopAll { fade_out_millis } => RuntimeAudioCommand::StopAll {
                fade_out_millis: map(fade_out_millis)?,
            },
            Self::SetVoiceGain {
                voice,
                gain_db_milli,
                transition_millis,
            } => RuntimeAudioCommand::SetVoiceGain {
                voice: map(voice)?,
                gain_db_milli: map(gain_db_milli)?,
                transition_millis: map(transition_millis)?,
            },
            Self::SetVoicePan {
                voice,
                pan_milli,
                transition_millis,
            } => RuntimeAudioCommand::SetVoicePan {
                voice: map(voice)?,
                pan_milli: map(pan_milli)?,
                transition_millis: map(transition_millis)?,
            },
            Self::SetBusGain {
                bus,
                gain_db_milli,
                transition_millis,
            } => RuntimeAudioCommand::SetBusGain {
                bus: map(bus)?,
                gain_db_milli: map(gain_db_milli)?,
                transition_millis: map(transition_millis)?,
            },
            Self::SetBusMute { bus, muted } => RuntimeAudioCommand::SetBusMute {
                bus: map(bus)?,
                muted: map(muted)?,
            },
            Self::SetEffectEnabled {
                bus,
                effect,
                enabled,
            } => RuntimeAudioCommand::SetEffectEnabled {
                bus: map(bus)?,
                effect: map(effect)?,
                enabled: map(enabled)?,
            },
            Self::SetEffectParameter {
                bus,
                effect,
                parameter,
                value,
                transition_millis,
            } => RuntimeAudioCommand::SetEffectParameter {
                bus: map(bus)?,
                effect: map(effect)?,
                parameter,
                value: map(value)?,
                transition_millis: map(transition_millis)?,
            },
            Self::ApplySnapshot {
                snapshot,
                transition_millis,
            } => RuntimeAudioCommand::ApplySnapshot {
                snapshot: map(snapshot)?,
                transition_millis: map(transition_millis)?,
            },
            Self::RequestMicrophone {
                capture,
                constraints,
            } => RuntimeAudioCommand::RequestMicrophone {
                capture: map(capture)?,
                constraints,
            },
            Self::StopMicrophone { capture } => RuntimeAudioCommand::StopMicrophone {
                capture: map(capture)?,
            },
            Self::SetCaptureMonitor {
                capture,
                bus,
                gain_db_milli,
            } => RuntimeAudioCommand::SetCaptureMonitor {
                capture: map(capture)?,
                bus: bus.map(&mut map).transpose()?,
                gain_db_milli: map(gain_db_milli)?,
            },
        })
    }

    #[must_use]
    pub const fn operation_name(&self) -> &'static str {
        match self {
            Self::Play { .. } => "play",
            Self::Stop { .. } => "stop",
            Self::StopAll { .. } => "stop_all",
            Self::SetVoiceGain { .. } => "set_voice_gain",
            Self::SetVoicePan { .. } => "set_voice_pan",
            Self::SetBusGain { .. } => "set_bus_gain",
            Self::SetBusMute { .. } => "set_bus_mute",
            Self::SetEffectEnabled { .. } => "set_effect_enabled",
            Self::SetEffectParameter { .. } => "set_effect_parameter",
            Self::ApplySnapshot { .. } => "apply_snapshot",
            Self::RequestMicrophone { .. } => "request_microphone",
            Self::StopMicrophone { .. } => "stop_microphone",
            Self::SetCaptureMonitor { .. } => "set_capture_monitor",
        }
    }
}

impl RuntimeAudioCommand {
    /// Direct owned expressions in the command ABI order.
    pub(crate) fn argument_exprs(&self) -> Vec<&RuntimeExpr> {
        let mut expressions = Vec::new();
        let mapped = self.expression_refs().try_map_expressions(|expr| {
            expressions.push(expr);
            Ok::<_, std::convert::Infallible>(())
        });
        match mapped {
            Ok(_) => expressions,
            Err(never) => match never {},
        }
    }
}
impl RuntimeAudioCommand {
    /// Non-expression metadata; owned argument expressions keep their shared ABI traversal.
    pub(crate) fn encode_body_metadata(
        &self,
        encoder: &mut crate::task::semantic::TaskSemanticEncoder<'_>,
    ) {
        encoder.tag(match self {
            Self::Play { .. } => 0,
            Self::Stop { .. } => 1,
            Self::StopAll { .. } => 2,
            Self::SetVoiceGain { .. } => 3,
            Self::SetVoicePan { .. } => 4,
            Self::SetBusGain { .. } => 5,
            Self::SetBusMute { .. } => 6,
            Self::SetEffectEnabled { .. } => 7,
            Self::SetEffectParameter { .. } => 8,
            Self::ApplySnapshot { .. } => 9,
            Self::RequestMicrophone { .. } => 10,
            Self::StopMicrophone { .. } => 11,
            Self::SetCaptureMonitor { .. } => 12,
        });
        match self {
            Self::Play { loop_mode, .. } => match loop_mode {
                AudioLoopMode::None => encoder.tag(0),
                AudioLoopMode::Whole => encoder.tag(1),
                AudioLoopMode::Region {
                    start_frame,
                    end_frame,
                } => {
                    encoder.tag(2);
                    encoder.scalar_u64(*start_frame);
                    encoder.scalar_u64(*end_frame);
                }
            },
            Self::SetEffectParameter { parameter, .. } => encoder.tag(parameter.semantic_tag()),
            Self::RequestMicrophone { constraints, .. } => {
                encoder.ordinal(u32::from(constraints.channels));
                encoder.tag(u8::from(constraints.preferred_sample_rate_hz.is_some()));
                if let Some(rate) = constraints.preferred_sample_rate_hz {
                    encoder.ordinal(rate);
                }
                encoder.tag(u8::from(constraints.echo_cancellation));
                encoder.tag(u8::from(constraints.noise_suppression));
                encoder.tag(u8::from(constraints.auto_gain_control));
            }
            Self::SetCaptureMonitor { bus, .. } => encoder.tag(u8::from(bus.is_some())),
            Self::Stop { .. }
            | Self::StopAll { .. }
            | Self::SetVoiceGain { .. }
            | Self::SetVoicePan { .. }
            | Self::SetBusGain { .. }
            | Self::SetBusMute { .. }
            | Self::SetEffectEnabled { .. }
            | Self::ApplySnapshot { .. }
            | Self::StopMicrophone { .. } => {}
        }
    }
}

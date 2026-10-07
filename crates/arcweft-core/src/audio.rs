use crate::value::RuntimeExpr;
use arcweft_interaction_model::audio::{
    AudioEffectParameterKind, AudioLoopMode, MicrophoneConstraints,
};

/// Typed runtime IR for audio commands whose values are evaluated by `Engine`.
#[allow(clippy::large_enum_variant)]
#[derive(Clone, Debug, PartialEq)]
pub enum RuntimeAudioCommand {
    Play {
        voice: RuntimeExpr,
        resource: RuntimeExpr,
        bus: RuntimeExpr,
        gain_db_milli: RuntimeExpr,
        pan_milli: RuntimeExpr,
        loop_mode: AudioLoopMode,
        start_frame: RuntimeExpr,
        fade_in_millis: RuntimeExpr,
    },
    Stop {
        voice: RuntimeExpr,
        fade_out_millis: RuntimeExpr,
    },
    StopAll {
        fade_out_millis: RuntimeExpr,
    },
    SetVoiceGain {
        voice: RuntimeExpr,
        gain_db_milli: RuntimeExpr,
        transition_millis: RuntimeExpr,
    },
    SetVoicePan {
        voice: RuntimeExpr,
        pan_milli: RuntimeExpr,
        transition_millis: RuntimeExpr,
    },
    SetBusGain {
        bus: RuntimeExpr,
        gain_db_milli: RuntimeExpr,
        transition_millis: RuntimeExpr,
    },
    SetBusMute {
        bus: RuntimeExpr,
        muted: RuntimeExpr,
    },
    SetEffectEnabled {
        bus: RuntimeExpr,
        effect: RuntimeExpr,
        enabled: RuntimeExpr,
    },
    SetEffectParameter {
        bus: RuntimeExpr,
        effect: RuntimeExpr,
        parameter: AudioEffectParameterKind,
        value: RuntimeExpr,
        transition_millis: RuntimeExpr,
    },
    ApplySnapshot {
        snapshot: RuntimeExpr,
        transition_millis: RuntimeExpr,
    },
    RequestMicrophone {
        capture: RuntimeExpr,
        constraints: MicrophoneConstraints,
    },
    StopMicrophone {
        capture: RuntimeExpr,
    },
    SetCaptureMonitor {
        capture: RuntimeExpr,
        bus: Option<RuntimeExpr>,
        gain_db_milli: RuntimeExpr,
    },
}

impl RuntimeAudioCommand {
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
        match self {
            Self::Play {
                voice,
                resource,
                bus,
                gain_db_milli,
                pan_milli,
                start_frame,
                fade_in_millis,
                ..
            } => vec![
                voice,
                resource,
                bus,
                gain_db_milli,
                pan_milli,
                start_frame,
                fade_in_millis,
            ],
            Self::Stop {
                voice,
                fade_out_millis,
            } => vec![voice, fade_out_millis],
            Self::StopAll { fade_out_millis } => vec![fade_out_millis],
            Self::SetVoiceGain {
                voice,
                gain_db_milli,
                transition_millis,
            } => vec![voice, gain_db_milli, transition_millis],
            Self::SetVoicePan {
                voice,
                pan_milli,
                transition_millis,
            } => vec![voice, pan_milli, transition_millis],
            Self::SetBusGain {
                bus,
                gain_db_milli,
                transition_millis,
            } => vec![bus, gain_db_milli, transition_millis],
            Self::SetBusMute { bus, muted } => vec![bus, muted],
            Self::SetEffectEnabled {
                bus,
                effect,
                enabled,
            } => vec![bus, effect, enabled],
            Self::SetEffectParameter {
                bus,
                effect,
                value,
                transition_millis,
                ..
            } => vec![bus, effect, value, transition_millis],
            Self::ApplySnapshot {
                snapshot,
                transition_millis,
            } => vec![snapshot, transition_millis],
            Self::RequestMicrophone { capture, .. } | Self::StopMicrophone { capture } => {
                vec![capture]
            }
            Self::SetCaptureMonitor {
                capture,
                bus,
                gain_db_milli,
            } => std::iter::once(capture)
                .chain(bus.iter())
                .chain(std::iter::once(gain_db_milli))
                .collect(),
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

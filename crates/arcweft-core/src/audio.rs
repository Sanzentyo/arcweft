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

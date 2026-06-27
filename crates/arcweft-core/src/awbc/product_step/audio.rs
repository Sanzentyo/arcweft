use super::ProductStepError;
use crate::awbc::schema::{AwbcAudioCommand, AwbcAudioValueRef, AwbcProgram};
use crate::awbc::vm::constant_value;
use crate::value::{RuntimeValue, runtime_value_label};
use arcweft_interaction_model::audio::{
    AudioBusId, AudioCaptureId, AudioCommand, AudioEffectId, AudioMillis, AudioResourceId,
    AudioSnapshotId, AudioVoiceId, GainDbMilli, PanMilli,
};

impl AwbcAudioCommand {
    #[allow(
        clippy::too_many_lines,
        reason = "Audio product mapping mirrors each stable command variant and field contract."
    )]
    pub(super) fn map_product_audio(
        &self,
        program: &AwbcProgram,
        args: &[RuntimeValue],
    ) -> Result<AudioCommand, ProductStepError> {
        let ctx = AudioPayloadContext { program, args };
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
            } => Ok(AudioCommand::Play {
                voice: ctx.voice(*voice, "play.voice")?,
                resource: ctx.resource(*resource, "play.resource")?,
                bus: ctx.bus(*bus, "play.bus")?,
                gain: ctx.gain(*gain_db_milli, "play.gain_db_milli")?,
                pan: ctx.pan(*pan_milli, "play.pan_milli")?,
                loop_mode: *loop_mode,
                start_frame: ctx.u64(*start_frame, "play.start_frame")?,
                fade_in: ctx.millis(*fade_in_millis, "play.fade_in_millis")?,
            }),
            Self::Stop {
                voice,
                fade_out_millis,
            } => Ok(AudioCommand::Stop {
                voice: ctx.voice(*voice, "stop.voice")?,
                fade_out: ctx.millis(*fade_out_millis, "stop.fade_out_millis")?,
            }),
            Self::StopAll { fade_out_millis } => Ok(AudioCommand::StopAll {
                fade_out: ctx.millis(*fade_out_millis, "stop_all.fade_out_millis")?,
            }),
            Self::SetVoiceGain {
                voice,
                gain_db_milli,
                transition_millis,
            } => Ok(AudioCommand::SetVoiceGain {
                voice: ctx.voice(*voice, "set_voice_gain.voice")?,
                gain: ctx.gain(*gain_db_milli, "set_voice_gain.gain_db_milli")?,
                transition: ctx.millis(*transition_millis, "set_voice_gain.transition_millis")?,
            }),
            Self::SetVoicePan {
                voice,
                pan_milli,
                transition_millis,
            } => Ok(AudioCommand::SetVoicePan {
                voice: ctx.voice(*voice, "set_voice_pan.voice")?,
                pan: ctx.pan(*pan_milli, "set_voice_pan.pan_milli")?,
                transition: ctx.millis(*transition_millis, "set_voice_pan.transition_millis")?,
            }),
            Self::SetBusGain {
                bus,
                gain_db_milli,
                transition_millis,
            } => Ok(AudioCommand::SetBusGain {
                bus: ctx.bus(*bus, "set_bus_gain.bus")?,
                gain: ctx.gain(*gain_db_milli, "set_bus_gain.gain_db_milli")?,
                transition: ctx.millis(*transition_millis, "set_bus_gain.transition_millis")?,
            }),
            Self::SetBusMute { bus, muted } => Ok(AudioCommand::SetBusMute {
                bus: ctx.bus(*bus, "set_bus_mute.bus")?,
                muted: ctx.bool(*muted, "set_bus_mute.muted")?,
            }),
            Self::SetEffectEnabled {
                bus,
                effect,
                enabled,
            } => Ok(AudioCommand::SetEffectEnabled {
                bus: ctx.bus(*bus, "set_effect_enabled.bus")?,
                effect: ctx.effect(*effect, "set_effect_enabled.effect")?,
                enabled: ctx.bool(*enabled, "set_effect_enabled.enabled")?,
            }),
            Self::SetEffectParameter {
                bus,
                effect,
                parameter,
                value,
                transition_millis,
            } => Ok(AudioCommand::SetEffectParameter {
                bus: ctx.bus(*bus, "set_effect_parameter.bus")?,
                effect: ctx.effect(*effect, "set_effect_parameter.effect")?,
                parameter: parameter
                    .from_i64(ctx.i64(*value, "set_effect_parameter.value")?)
                    .map_err(|error| ProductStepError::Type(error.to_string()))?,
                transition: ctx
                    .millis(*transition_millis, "set_effect_parameter.transition_millis")?,
            }),
            Self::ApplySnapshot {
                snapshot,
                transition_millis,
            } => Ok(AudioCommand::ApplySnapshot {
                snapshot: ctx.snapshot(*snapshot, "apply_snapshot.snapshot")?,
                transition: ctx.millis(*transition_millis, "apply_snapshot.transition_millis")?,
            }),
            Self::RequestMicrophone {
                capture,
                constraints,
            } => Ok(AudioCommand::RequestMicrophone {
                capture: ctx.capture(*capture, "request_microphone.capture")?,
                constraints: constraints
                    .validate()
                    .map_err(|error| ProductStepError::Type(error.to_string()))?,
            }),
            Self::StopMicrophone { capture } => Ok(AudioCommand::StopMicrophone {
                capture: ctx.capture(*capture, "stop_microphone.capture")?,
            }),
            Self::SetCaptureMonitor {
                capture,
                bus,
                gain_db_milli,
            } => Ok(AudioCommand::SetCaptureMonitor {
                capture: ctx.capture(*capture, "set_capture_monitor.capture")?,
                bus: bus
                    .map(|bus| ctx.bus(bus, "set_capture_monitor.bus"))
                    .transpose()?,
                gain: ctx.gain(*gain_db_milli, "set_capture_monitor.gain_db_milli")?,
            }),
        }
    }
}

struct AudioPayloadContext<'a> {
    program: &'a AwbcProgram,
    args: &'a [RuntimeValue],
}

impl AudioPayloadContext<'_> {
    fn value(
        &self,
        value: AwbcAudioValueRef,
        field: &str,
    ) -> Result<RuntimeValue, ProductStepError> {
        match value {
            AwbcAudioValueRef::Arg(arg) => self.args.get(arg.index()).cloned().ok_or_else(|| {
                ProductStepError::Internal(format!(
                    "AWBC audio field `{field}` references missing dynamic arg {}",
                    arg.0
                ))
            }),
            AwbcAudioValueRef::Const(constant) => constant_value(self.program, constant)
                .map_err(|error| ProductStepError::Internal(error.to_string())),
        }
    }

    fn identifier(
        &self,
        value: AwbcAudioValueRef,
        field: &str,
    ) -> Result<String, ProductStepError> {
        let value = self.value(value, field)?;
        value.as_identifier().map(str::to_owned).ok_or_else(|| {
            ProductStepError::Type(format!(
                "AWBC audio field `{field}` expected audio identifier, found {}",
                runtime_value_label(&value)
            ))
        })
    }

    fn voice(
        &self,
        value: AwbcAudioValueRef,
        field: &str,
    ) -> Result<AudioVoiceId, ProductStepError> {
        let value = self.identifier(value, field)?;
        AudioVoiceId::new(value.clone()).map_err(|error| {
            ProductStepError::Type(format!("invalid AWBC audio voice `{value}`: {error}"))
        })
    }

    fn resource(
        &self,
        value: AwbcAudioValueRef,
        field: &str,
    ) -> Result<AudioResourceId, ProductStepError> {
        let value = self.identifier(value, field)?;
        AudioResourceId::new(value.clone()).map_err(|error| {
            ProductStepError::Type(format!("invalid AWBC audio resource `{value}`: {error}"))
        })
    }

    fn bus(&self, value: AwbcAudioValueRef, field: &str) -> Result<AudioBusId, ProductStepError> {
        let value = self.identifier(value, field)?;
        AudioBusId::new(value.clone()).map_err(|error| {
            ProductStepError::Type(format!("invalid AWBC audio bus `{value}`: {error}"))
        })
    }

    fn effect(
        &self,
        value: AwbcAudioValueRef,
        field: &str,
    ) -> Result<AudioEffectId, ProductStepError> {
        let value = self.identifier(value, field)?;
        AudioEffectId::new(value.clone()).map_err(|error| {
            ProductStepError::Type(format!("invalid AWBC audio effect `{value}`: {error}"))
        })
    }

    fn snapshot(
        &self,
        value: AwbcAudioValueRef,
        field: &str,
    ) -> Result<AudioSnapshotId, ProductStepError> {
        let value = self.identifier(value, field)?;
        AudioSnapshotId::new(value.clone()).map_err(|error| {
            ProductStepError::Type(format!("invalid AWBC audio snapshot `{value}`: {error}"))
        })
    }

    fn capture(
        &self,
        value: AwbcAudioValueRef,
        field: &str,
    ) -> Result<AudioCaptureId, ProductStepError> {
        let value = self.identifier(value, field)?;
        AudioCaptureId::new(value.clone()).map_err(|error| {
            ProductStepError::Type(format!("invalid AWBC audio capture `{value}`: {error}"))
        })
    }

    fn bool(&self, value: AwbcAudioValueRef, field: &str) -> Result<bool, ProductStepError> {
        let value = self.value(value, field)?;
        value.as_bool().ok_or_else(|| {
            ProductStepError::Type(format!(
                "AWBC audio field `{field}` expected bool, found {}",
                runtime_value_label(&value)
            ))
        })
    }

    fn i64(&self, value: AwbcAudioValueRef, field: &str) -> Result<i64, ProductStepError> {
        let value = self.value(value, field)?;
        value.try_i64().ok_or_else(|| {
            ProductStepError::Type(format!(
                "AWBC audio field `{field}` expected integer, found {}",
                runtime_value_label(&value)
            ))
        })
    }

    fn u64(&self, value: AwbcAudioValueRef, field: &str) -> Result<u64, ProductStepError> {
        let value = self.value(value, field)?;
        value.try_u64().ok_or_else(|| {
            ProductStepError::Type(format!(
                "AWBC audio field `{field}` expected unsigned integer, found {}",
                runtime_value_label(&value)
            ))
        })
    }

    fn u32(&self, value: AwbcAudioValueRef, field: &str) -> Result<u32, ProductStepError> {
        let raw = self.u64(value, field)?;
        u32::try_from(raw).map_err(|_| {
            ProductStepError::Type(format!(
                "AWBC audio field `{field}` expected u32-compatible integer, found {raw}"
            ))
        })
    }

    fn i32(&self, value: AwbcAudioValueRef, field: &str) -> Result<i32, ProductStepError> {
        let raw = self.i64(value, field)?;
        i32::try_from(raw).map_err(|_| {
            ProductStepError::Type(format!(
                "AWBC audio field `{field}` expected i32-compatible integer, found {raw}"
            ))
        })
    }

    fn i16(&self, value: AwbcAudioValueRef, field: &str) -> Result<i16, ProductStepError> {
        let raw = self.i64(value, field)?;
        i16::try_from(raw).map_err(|_| {
            ProductStepError::Type(format!(
                "AWBC audio field `{field}` expected i16-compatible integer, found {raw}"
            ))
        })
    }

    fn millis(
        &self,
        value: AwbcAudioValueRef,
        field: &str,
    ) -> Result<AudioMillis, ProductStepError> {
        self.u32(value, field).map(AudioMillis::new)
    }

    fn gain(&self, value: AwbcAudioValueRef, field: &str) -> Result<GainDbMilli, ProductStepError> {
        GainDbMilli::new(self.i32(value, field)?)
            .map_err(|error| ProductStepError::Type(error.to_string()))
    }

    fn pan(&self, value: AwbcAudioValueRef, field: &str) -> Result<PanMilli, ProductStepError> {
        PanMilli::new(self.i16(value, field)?)
            .map_err(|error| ProductStepError::Type(error.to_string()))
    }
}

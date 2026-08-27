use super::AwbcCodecError;
use super::wire::{Reader, Wire, Writer, wire_enum};
use crate::awbc::schema::{
    AwbcAudioArg, AwbcAudioCleanup, AwbcAudioCommand, AwbcAudioCommandId, AwbcAudioValueRef,
    AwbcAwaitManyPolicy, AwbcChildCancelPolicy, AwbcChildCleanup, AwbcChildJoinPolicy, AwbcChoice,
    AwbcChoiceOption, AwbcConflictPolicy, AwbcConstantId, AwbcEffectKind, AwbcEffectPlan,
    AwbcEffectPlanId, AwbcFunctionId, AwbcHostArgument, AwbcHostCall, AwbcHostCallMode,
    AwbcIntrinsic, AwbcLineCancelHandler, AwbcLineCleanupPolicy, AwbcLineHandleSite,
    AwbcLineHandleSiteId, AwbcLineOperation, AwbcLineTaskGroup, AwbcLineTaskGroupId,
    AwbcLineTaskNode, AwbcLineTaskNodeId, AwbcLineTaskTrigger, AwbcParallelPolicy,
    AwbcPresentationCleanup, AwbcPureHelper, AwbcPureHelperOrigin, AwbcReduceOp, AwbcRegisterId,
    AwbcResourceAccess, AwbcResourceAccessMode, AwbcResourceId, AwbcSignatureId, AwbcStreamPlan,
    AwbcStringId, AwbcTableRange, AwbcTaskClass, AwbcTaskPlan, AwbcTaskPolicy, AwbcTypeId,
};
use crate::runtime_id::{
    RuntimeDialogueEffectSiteId, RuntimeDialogueMarkId, RuntimeLocalDeclarationId,
};
use crate::value::{RuntimeCallTarget, RuntimeIntrinsic};
use arcweft_character::id::CharacterId;
use arcweft_interaction_model::audio::{
    AudioEffectParameterKind, AudioLoopMode, MicrophoneConstraints,
};
use std::num::NonZeroU32;

impl Wire for AwbcIntrinsic {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        self.identity.write_wire(writer)?;
        self.signature.write_wire(writer)?;
        self.revision.write_wire(writer)
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        Ok(Self {
            identity: RuntimeCallTarget::read_wire(reader)?,
            signature: AwbcSignatureId::read_wire(reader)?,
            revision: u32::read_wire(reader)?,
        })
    }
}

impl Wire for RuntimeCallTarget {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        match self {
            Self::Intrinsic(intrinsic) => {
                0_u8.write_wire(writer)?;
                intrinsic.as_label().to_owned().write_wire(writer)
            }
            Self::Callable(callable) => {
                1_u8.write_wire(writer)?;
                callable.write_wire(writer)
            }
        }
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        let offset = reader.offset();
        match u8::read_wire(reader)? {
            0 => {
                let label = String::read_wire(reader)?;
                RuntimeIntrinsic::from_label(&label)
                    .map(Self::Intrinsic)
                    .ok_or(AwbcCodecError::InvalidMetadata {
                        kind: "runtime intrinsic identity",
                        message: format!("unknown runtime intrinsic `{label}`"),
                        offset,
                    })
            }
            1 => crate::entry::RuntimeCallableId::read_wire(reader).map(Self::Callable),
            tag => Err(AwbcCodecError::UnknownTag {
                kind: "runtime call target",
                tag,
                offset,
            }),
        }
    }
}

impl Wire for AwbcHostCall {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        self.public_id.write_wire(writer)?;
        self.capability.write_wire(writer)?;
        self.operation.write_wire(writer)?;
        self.contract.write_wire(writer)?;
        self.signature.write_wire(writer)?;
        self.mode.write_wire(writer)?;
        self.deterministic.write_wire(writer)?;
        self.arguments.write_wire(writer)
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        Ok(Self {
            public_id: AwbcStringId::read_wire(reader)?,
            capability: AwbcStringId::read_wire(reader)?,
            operation: AwbcStringId::read_wire(reader)?,
            contract: Option::<crate::step::HostCallContractDigest>::read_wire(reader)?,
            signature: AwbcSignatureId::read_wire(reader)?,
            mode: AwbcHostCallMode::read_wire(reader)?,
            deterministic: bool::read_wire(reader)?,
            arguments: Vec::<AwbcHostArgument>::read_wire(reader)?,
        })
    }
}

impl Wire for crate::step::HostCallContractDigest {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        self.as_bytes().write_wire(writer)
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        <[u8; 32]>::read_wire(reader).map(Self::from_bytes)
    }
}

wire_enum!(AwbcHostCallMode, "host call mode", {
    0 => AwbcHostCallMode::Immediate,
    1 => AwbcHostCallMode::Suspend,
});

impl Wire for AwbcTaskPlan {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        self.public_id.write_wire(writer)?;
        self.need_id.write_wire(writer)?;
        self.capability.write_wire(writer)?;
        self.operation.write_wire(writer)?;
        self.signature.write_wire(writer)?;
        self.class.write_wire(writer)?;
        self.priority.write_wire(writer)?;
        self.cancel_scope.write_wire(writer)?;
        self.policy.write_wire(writer)?;
        self.payload_type.write_wire(writer)?;
        self.arguments.write_wire(writer)?;
        self.many.write_wire(writer)
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        Ok(Self {
            public_id: AwbcStringId::read_wire(reader)?,
            need_id: AwbcStringId::read_wire(reader)?,
            capability: AwbcStringId::read_wire(reader)?,
            operation: AwbcStringId::read_wire(reader)?,
            signature: AwbcSignatureId::read_wire(reader)?,
            class: AwbcTaskClass::read_wire(reader)?,
            priority: i32::read_wire(reader)?,
            cancel_scope: AwbcStringId::read_wire(reader)?,
            policy: AwbcTaskPolicy::read_wire(reader)?,
            payload_type: AwbcTypeId::read_wire(reader)?,
            arguments: Vec::<AwbcHostArgument>::read_wire(reader)?,
            many: Option::<AwbcAwaitManyPolicy>::read_wire(reader)?,
        })
    }
}

wire_enum!(AwbcTaskClass, "task class", {
    0 => AwbcTaskClass::LocalView,
    1 => AwbcTaskClass::Io,
    2 => AwbcTaskClass::Cpu,
    3 => AwbcTaskClass::GpuPrepare,
    4 => AwbcTaskClass::ShaderCompile,
    5 => AwbcTaskClass::WasmCall,
    6 => AwbcTaskClass::AssetDecode,
    7 => AwbcTaskClass::AudioDecode,
    8 => AwbcTaskClass::AudioRender,
    9 => AwbcTaskClass::TtsSynthesis,
    10 => AwbcTaskClass::BgmPrecompose,
    11 => AwbcTaskClass::Lsp,
    12 => AwbcTaskClass::Background,
});

wire_enum!(AwbcTaskPolicy, "task policy", {
    0 => AwbcTaskPolicy::JoinSameKey,
    1 => AwbcTaskPolicy::AlwaysStart,
});

impl Wire for AwbcHostArgument {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        self.name.write_wire(writer)?;
        self.spread.write_wire(writer)
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        Ok(Self {
            name: Option::<AwbcStringId>::read_wire(reader)?,
            spread: bool::read_wire(reader)?,
        })
    }
}

impl Wire for AwbcAwaitManyPolicy {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        self.item_binding.write_wire(writer)?;
        self.limit.write_wire(writer)
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        Ok(Self {
            item_binding: AwbcRegisterId::read_wire(reader)?,
            limit: u32::read_wire(reader)?,
        })
    }
}

impl Wire for AwbcAudioArg {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        self.0.write_wire(writer)
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        u32::read_wire(reader).map(Self)
    }
}

impl Wire for AwbcAudioValueRef {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        match self {
            Self::Arg(value) => {
                writer.write_u8(0);
                value.write_wire(writer)?;
            }
            Self::Const(value) => {
                writer.write_u8(1);
                value.write_wire(writer)?;
            }
        }
        Ok(())
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        let offset = reader.offset();
        Ok(match reader.read_u8()? {
            0 => Self::Arg(AwbcAudioArg::read_wire(reader)?),
            1 => Self::Const(AwbcConstantId::read_wire(reader)?),
            tag => {
                return Err(AwbcCodecError::UnknownTag {
                    kind: "audio value ref",
                    tag,
                    offset,
                });
            }
        })
    }
}

impl Wire for AudioLoopMode {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        match self {
            Self::None => writer.write_u8(0),
            Self::Whole => writer.write_u8(1),
            Self::Region {
                start_frame,
                end_frame,
            } => {
                writer.write_u8(2);
                start_frame.write_wire(writer)?;
                end_frame.write_wire(writer)?;
            }
        }
        Ok(())
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        let offset = reader.offset();
        Ok(match reader.read_u8()? {
            0 => Self::None,
            1 => Self::Whole,
            2 => Self::Region {
                start_frame: u64::read_wire(reader)?,
                end_frame: u64::read_wire(reader)?,
            },
            tag => {
                return Err(AwbcCodecError::UnknownTag {
                    kind: "audio loop mode",
                    tag,
                    offset,
                });
            }
        })
    }
}

wire_enum!(AudioEffectParameterKind, "audio effect parameter kind", {
    0 => AudioEffectParameterKind::BiquadCutoffMilliHz,
    1 => AudioEffectParameterKind::BiquadQMilli,
    2 => AudioEffectParameterKind::CompressorThresholdDbMilli,
    3 => AudioEffectParameterKind::CompressorRatioMilli,
    4 => AudioEffectParameterKind::CompressorAttackMicros,
    5 => AudioEffectParameterKind::CompressorReleaseMicros,
    6 => AudioEffectParameterKind::CompressorMakeupDbMilli,
    7 => AudioEffectParameterKind::DelayTimeMillis,
    8 => AudioEffectParameterKind::DelayFeedbackMilli,
    9 => AudioEffectParameterKind::ReverbRoomSizeMilli,
    10 => AudioEffectParameterKind::ReverbDampingMilli,
    11 => AudioEffectParameterKind::WetGainDbMilli,
    12 => AudioEffectParameterKind::DryGainDbMilli,
    13 => AudioEffectParameterKind::LimiterCeilingDbMilli,
    14 => AudioEffectParameterKind::LimiterReleaseMicros,
});

impl Wire for MicrophoneConstraints {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        self.channels.write_wire(writer)?;
        self.preferred_sample_rate_hz.write_wire(writer)?;
        self.echo_cancellation.write_wire(writer)?;
        self.noise_suppression.write_wire(writer)?;
        self.auto_gain_control.write_wire(writer)
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        Ok(Self {
            channels: u16::read_wire(reader)?,
            preferred_sample_rate_hz: Option::<u32>::read_wire(reader)?,
            echo_cancellation: bool::read_wire(reader)?,
            noise_suppression: bool::read_wire(reader)?,
            auto_gain_control: bool::read_wire(reader)?,
        })
    }
}

impl Wire for AwbcAudioCommand {
    #[allow(
        clippy::too_many_lines,
        reason = "Audio command wire order mirrors the stable payload enum one variant at a time."
    )]
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
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
            } => {
                writer.write_u8(0);
                voice.write_wire(writer)?;
                resource.write_wire(writer)?;
                bus.write_wire(writer)?;
                gain_db_milli.write_wire(writer)?;
                pan_milli.write_wire(writer)?;
                loop_mode.write_wire(writer)?;
                start_frame.write_wire(writer)?;
                fade_in_millis.write_wire(writer)?;
            }
            Self::Stop {
                voice,
                fade_out_millis,
            } => {
                writer.write_u8(1);
                voice.write_wire(writer)?;
                fade_out_millis.write_wire(writer)?;
            }
            Self::StopAll { fade_out_millis } => {
                writer.write_u8(2);
                fade_out_millis.write_wire(writer)?;
            }
            Self::SetVoiceGain {
                voice,
                gain_db_milli,
                transition_millis,
            } => {
                writer.write_u8(3);
                voice.write_wire(writer)?;
                gain_db_milli.write_wire(writer)?;
                transition_millis.write_wire(writer)?;
            }
            Self::SetVoicePan {
                voice,
                pan_milli,
                transition_millis,
            } => {
                writer.write_u8(4);
                voice.write_wire(writer)?;
                pan_milli.write_wire(writer)?;
                transition_millis.write_wire(writer)?;
            }
            Self::SetBusGain {
                bus,
                gain_db_milli,
                transition_millis,
            } => {
                writer.write_u8(5);
                bus.write_wire(writer)?;
                gain_db_milli.write_wire(writer)?;
                transition_millis.write_wire(writer)?;
            }
            Self::SetBusMute { bus, muted } => {
                writer.write_u8(6);
                bus.write_wire(writer)?;
                muted.write_wire(writer)?;
            }
            Self::SetEffectEnabled {
                bus,
                effect,
                enabled,
            } => {
                writer.write_u8(7);
                bus.write_wire(writer)?;
                effect.write_wire(writer)?;
                enabled.write_wire(writer)?;
            }
            Self::SetEffectParameter {
                bus,
                effect,
                parameter,
                value,
                transition_millis,
            } => {
                writer.write_u8(8);
                bus.write_wire(writer)?;
                effect.write_wire(writer)?;
                parameter.write_wire(writer)?;
                value.write_wire(writer)?;
                transition_millis.write_wire(writer)?;
            }
            Self::ApplySnapshot {
                snapshot,
                transition_millis,
            } => {
                writer.write_u8(9);
                snapshot.write_wire(writer)?;
                transition_millis.write_wire(writer)?;
            }
            Self::RequestMicrophone {
                capture,
                constraints,
            } => {
                writer.write_u8(10);
                capture.write_wire(writer)?;
                constraints.write_wire(writer)?;
            }
            Self::StopMicrophone { capture } => {
                writer.write_u8(11);
                capture.write_wire(writer)?;
            }
            Self::SetCaptureMonitor {
                capture,
                bus,
                gain_db_milli,
            } => {
                writer.write_u8(12);
                capture.write_wire(writer)?;
                bus.write_wire(writer)?;
                gain_db_milli.write_wire(writer)?;
            }
        }
        Ok(())
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        let offset = reader.offset();
        Ok(match reader.read_u8()? {
            0 => Self::Play {
                voice: AwbcAudioValueRef::read_wire(reader)?,
                resource: AwbcAudioValueRef::read_wire(reader)?,
                bus: AwbcAudioValueRef::read_wire(reader)?,
                gain_db_milli: AwbcAudioValueRef::read_wire(reader)?,
                pan_milli: AwbcAudioValueRef::read_wire(reader)?,
                loop_mode: AudioLoopMode::read_wire(reader)?,
                start_frame: AwbcAudioValueRef::read_wire(reader)?,
                fade_in_millis: AwbcAudioValueRef::read_wire(reader)?,
            },
            1 => Self::Stop {
                voice: AwbcAudioValueRef::read_wire(reader)?,
                fade_out_millis: AwbcAudioValueRef::read_wire(reader)?,
            },
            2 => Self::StopAll {
                fade_out_millis: AwbcAudioValueRef::read_wire(reader)?,
            },
            3 => Self::SetVoiceGain {
                voice: AwbcAudioValueRef::read_wire(reader)?,
                gain_db_milli: AwbcAudioValueRef::read_wire(reader)?,
                transition_millis: AwbcAudioValueRef::read_wire(reader)?,
            },
            4 => Self::SetVoicePan {
                voice: AwbcAudioValueRef::read_wire(reader)?,
                pan_milli: AwbcAudioValueRef::read_wire(reader)?,
                transition_millis: AwbcAudioValueRef::read_wire(reader)?,
            },
            5 => Self::SetBusGain {
                bus: AwbcAudioValueRef::read_wire(reader)?,
                gain_db_milli: AwbcAudioValueRef::read_wire(reader)?,
                transition_millis: AwbcAudioValueRef::read_wire(reader)?,
            },
            6 => Self::SetBusMute {
                bus: AwbcAudioValueRef::read_wire(reader)?,
                muted: AwbcAudioValueRef::read_wire(reader)?,
            },
            7 => Self::SetEffectEnabled {
                bus: AwbcAudioValueRef::read_wire(reader)?,
                effect: AwbcAudioValueRef::read_wire(reader)?,
                enabled: AwbcAudioValueRef::read_wire(reader)?,
            },
            8 => Self::SetEffectParameter {
                bus: AwbcAudioValueRef::read_wire(reader)?,
                effect: AwbcAudioValueRef::read_wire(reader)?,
                parameter: AudioEffectParameterKind::read_wire(reader)?,
                value: AwbcAudioValueRef::read_wire(reader)?,
                transition_millis: AwbcAudioValueRef::read_wire(reader)?,
            },
            9 => Self::ApplySnapshot {
                snapshot: AwbcAudioValueRef::read_wire(reader)?,
                transition_millis: AwbcAudioValueRef::read_wire(reader)?,
            },
            10 => Self::RequestMicrophone {
                capture: AwbcAudioValueRef::read_wire(reader)?,
                constraints: MicrophoneConstraints::read_wire(reader)?,
            },
            11 => Self::StopMicrophone {
                capture: AwbcAudioValueRef::read_wire(reader)?,
            },
            12 => Self::SetCaptureMonitor {
                capture: AwbcAudioValueRef::read_wire(reader)?,
                bus: Option::<AwbcAudioValueRef>::read_wire(reader)?,
                gain_db_milli: AwbcAudioValueRef::read_wire(reader)?,
            },
            tag => {
                return Err(AwbcCodecError::UnknownTag {
                    kind: "audio command",
                    tag,
                    offset,
                });
            }
        })
    }
}

impl Wire for AwbcEffectPlan {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        self.kind.write_wire(writer)?;
        self.signature.write_wire(writer)?;
        self.capability.write_wire(writer)?;
        self.audio.write_wire(writer)?;
        self.static_args.write_wire(writer)?;
        self.resources.write_wire(writer)
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        Ok(Self {
            kind: AwbcEffectKind::read_wire(reader)?,
            signature: AwbcSignatureId::read_wire(reader)?,
            capability: Option::<AwbcStringId>::read_wire(reader)?,
            audio: Option::<AwbcAudioCommandId>::read_wire(reader)?,
            static_args: Vec::<AwbcConstantId>::read_wire(reader)?,
            resources: Vec::<AwbcResourceAccess>::read_wire(reader)?,
        })
    }
}

wire_enum!(AwbcEffectKind, "effect kind", {
    0 => AwbcEffectKind::Wait,
    1 => AwbcEffectKind::Audio,
    2 => AwbcEffectKind::Call,
    3 => AwbcEffectKind::Log,
    4 => AwbcEffectKind::SignalWrite,
    5 => AwbcEffectKind::MetricWrite,
    6 => AwbcEffectKind::EmitEvent,
    7 => AwbcEffectKind::Out,
    8 => AwbcEffectKind::Return,
    9 => AwbcEffectKind::Goto,
    10 => AwbcEffectKind::Panic,
    11 => AwbcEffectKind::Fail,
    12 => AwbcEffectKind::Bail,
    13 => AwbcEffectKind::Ensure,
    14 => AwbcEffectKind::Assert,
    15 => AwbcEffectKind::Close,
    16 => AwbcEffectKind::Select,
    17 => AwbcEffectKind::Break,
    18 => AwbcEffectKind::Continue,
});

impl Wire for AwbcResourceAccess {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        self.resource.write_wire(writer)?;
        self.mode.write_wire(writer)?;
        self.conflict.write_wire(writer)
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        Ok(Self {
            resource: AwbcResourceId::read_wire(reader)?,
            mode: AwbcResourceAccessMode::read_wire(reader)?,
            conflict: AwbcConflictPolicy::read_wire(reader)?,
        })
    }
}

wire_enum!(AwbcResourceAccessMode, "resource access mode", {
    0 => AwbcResourceAccessMode::Read,
    1 => AwbcResourceAccessMode::Write,
    2 => AwbcResourceAccessMode::Drop,
    3 => AwbcResourceAccessMode::Append,
    4 => AwbcResourceAccessMode::Control,
});

impl Wire for AwbcConflictPolicy {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        match self {
            Self::Error => writer.write_u8(0),
            Self::Append => writer.write_u8(1),
            Self::LastWriterWins { priority } => {
                writer.write_u8(2);
                priority.write_wire(writer)?;
            }
            Self::MergePatch => writer.write_u8(3),
            Self::Reduce { op } => {
                writer.write_u8(4);
                op.write_wire(writer)?;
            }
        }
        Ok(())
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        let offset = reader.offset();
        Ok(match reader.read_u8()? {
            0 => Self::Error,
            1 => Self::Append,
            2 => Self::LastWriterWins {
                priority: i32::read_wire(reader)?,
            },
            3 => Self::MergePatch,
            4 => Self::Reduce {
                op: AwbcReduceOp::read_wire(reader)?,
            },
            tag => {
                return Err(AwbcCodecError::UnknownTag {
                    kind: "conflict policy",
                    tag,
                    offset,
                });
            }
        })
    }
}

wire_enum!(AwbcReduceOp, "reduce operator", {
    0 => AwbcReduceOp::Sum,
    1 => AwbcReduceOp::Min,
    2 => AwbcReduceOp::Max,
    3 => AwbcReduceOp::And,
    4 => AwbcReduceOp::Or,
});

impl Wire for AwbcChoice {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        self.public_id.write_wire(writer)?;
        self.options.write_wire(writer)
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        Ok(Self {
            public_id: Option::<AwbcStringId>::read_wire(reader)?,
            options: AwbcTableRange::read_wire(reader)?,
        })
    }
}

impl Wire for AwbcChoiceOption {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        self.public_id.write_wire(writer)?;
        self.label.write_wire(writer)?;
        self.condition.write_wire(writer)?;
        self.target.write_wire(writer)?;
        self.out_effect.write_wire(writer)?;
        self.effects.write_wire(writer)
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        Ok(Self {
            public_id: Option::<AwbcStringId>::read_wire(reader)?,
            label: AwbcStringId::read_wire(reader)?,
            condition: Option::<AwbcFunctionId>::read_wire(reader)?,
            target: Option::<AwbcFunctionId>::read_wire(reader)?,
            out_effect: Option::<AwbcEffectPlanId>::read_wire(reader)?,
            effects: Vec::<AwbcEffectPlanId>::read_wire(reader)?,
        })
    }
}

impl Wire for AwbcLineTaskGroup {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        self.captures.write_wire(writer)?;
        self.activation.write_wire(writer)?;
        self.result_type.write_wire(writer)?;
        self.handle_sites.write_wire(writer)?;
        self.root.write_wire(writer)?;
        self.nodes.write_wire(writer)?;
        self.cancel_handlers.write_wire(writer)?;
        self.cleanup_completed.write_wire(writer)?;
        self.cleanup_cancelled.write_wire(writer)?;
        self.cleanup_failed.write_wire(writer)?;
        self.cleanup.write_wire(writer)
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        Ok(Self {
            captures: Vec::<RuntimeLocalDeclarationId>::read_wire(reader)?,
            activation: AwbcFunctionId::read_wire(reader)?,
            result_type: AwbcTypeId::read_wire(reader)?,
            handle_sites: Vec::<AwbcLineHandleSite>::read_wire(reader)?,
            root: AwbcLineTaskNodeId::read_wire(reader)?,
            nodes: AwbcTableRange::read_wire(reader)?,
            cancel_handlers: Vec::<AwbcLineCancelHandler>::read_wire(reader)?,
            cleanup_completed: Option::<AwbcFunctionId>::read_wire(reader)?,
            cleanup_cancelled: Option::<AwbcFunctionId>::read_wire(reader)?,
            cleanup_failed: Option::<AwbcFunctionId>::read_wire(reader)?,
            cleanup: AwbcLineCleanupPolicy::read_wire(reader)?,
        })
    }
}

impl Wire for AwbcLineHandleSite {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        self.source_ordinal.write_wire(writer)?;
        self.kind.write_wire(writer)?;
        self.result_type.write_wire(writer)?;
        self.character.write_wire(writer)?;
        self.scheduled_child.write_wire(writer)
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        Ok(Self {
            source_ordinal: u32::read_wire(reader)?,
            kind: crate::value::RuntimeHandleKind::read_wire(reader)?,
            result_type: AwbcTypeId::read_wire(reader)?,
            character: Option::<CharacterId>::read_wire(reader)?,
            scheduled_child: Option::<AwbcLineTaskNodeId>::read_wire(reader)?,
        })
    }
}

impl Wire for crate::awbc::schema::AwbcLineScheduledCapture {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        self.local.write_wire(writer)?;
        self.ty.write_wire(writer)
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        Ok(Self {
            local: RuntimeLocalDeclarationId::read_wire(reader)?,
            ty: AwbcTypeId::read_wire(reader)?,
        })
    }
}

impl Wire for AwbcLineOperation {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        match self {
            Self::AcquireActor {
                group,
                site,
                character,
                scope,
                result_type,
            } => {
                writer.write_u8(0);
                group.write_wire(writer)?;
                site.write_wire(writer)?;
                character.write_wire(writer)?;
                match scope {
                    crate::line_task::RuntimeLineHandleScope::Line => writer.write_u8(0),
                }
                result_type.write_wire(writer)?;
            }
            Self::Schedule {
                group,
                site,
                child,
                captures,
                result_type,
            } => {
                writer.write_u8(1);
                group.write_wire(writer)?;
                site.write_wire(writer)?;
                child.write_wire(writer)?;
                captures.write_wire(writer)?;
                result_type.write_wire(writer)?;
            }
            Self::ActorLook {
                group,
                site,
                character,
                actor_type,
                look_type,
                result_type,
            } => {
                writer.write_u8(2);
                group.write_wire(writer)?;
                site.write_wire(writer)?;
                character.write_wire(writer)?;
                actor_type.write_wire(writer)?;
                look_type.write_wire(writer)?;
                result_type.write_wire(writer)?;
            }
            Self::VoiceHandle {
                group,
                site,
                result_type,
            } => {
                writer.write_u8(3);
                group.write_wire(writer)?;
                site.write_wire(writer)?;
                result_type.write_wire(writer)?;
            }
        }
        Ok(())
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        let offset = reader.offset();
        Ok(match reader.read_u8()? {
            0 => {
                let group = AwbcLineTaskGroupId::read_wire(reader)?;
                let site = AwbcLineHandleSiteId::read_wire(reader)?;
                let character = CharacterId::read_wire(reader)?;
                let scope_offset = reader.offset();
                let scope = match reader.read_u8()? {
                    0 => crate::line_task::RuntimeLineHandleScope::Line,
                    tag => {
                        return Err(AwbcCodecError::UnknownTag {
                            kind: "line handle scope",
                            tag,
                            offset: scope_offset,
                        });
                    }
                };
                Self::AcquireActor {
                    group,
                    site,
                    character,
                    scope,
                    result_type: AwbcTypeId::read_wire(reader)?,
                }
            }
            1 => Self::Schedule {
                group: AwbcLineTaskGroupId::read_wire(reader)?,
                site: AwbcLineHandleSiteId::read_wire(reader)?,
                child: AwbcLineTaskNodeId::read_wire(reader)?,
                captures: Vec::<crate::awbc::schema::AwbcLineScheduledCapture>::read_wire(reader)?,
                result_type: AwbcTypeId::read_wire(reader)?,
            },
            2 => Self::ActorLook {
                group: AwbcLineTaskGroupId::read_wire(reader)?,
                site: AwbcLineHandleSiteId::read_wire(reader)?,
                character: CharacterId::read_wire(reader)?,
                actor_type: AwbcTypeId::read_wire(reader)?,
                look_type: AwbcTypeId::read_wire(reader)?,
                result_type: AwbcTypeId::read_wire(reader)?,
            },
            3 => Self::VoiceHandle {
                group: AwbcLineTaskGroupId::read_wire(reader)?,
                site: AwbcLineHandleSiteId::read_wire(reader)?,
                result_type: AwbcTypeId::read_wire(reader)?,
            },
            tag => {
                return Err(AwbcCodecError::UnknownTag {
                    kind: "line operation",
                    tag,
                    offset,
                });
            }
        })
    }
}

impl Wire for CharacterId {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        self.as_str().to_owned().write_wire(writer)
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        let offset = reader.offset();
        CharacterId::try_new(String::read_wire(reader)?).map_err(|error| {
            AwbcCodecError::InvalidMetadata {
                kind: "Character identity",
                message: error.to_string(),
                offset,
            }
        })
    }
}

impl Wire for AwbcLineCancelHandler {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        self.trigger.get().get().write_wire(writer)?;
        self.function.write_wire(writer)
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        Ok(Self {
            trigger: runtime_dialogue_mark_id(reader)?,
            function: AwbcFunctionId::read_wire(reader)?,
        })
    }
}

impl Wire for AwbcLineCleanupPolicy {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        self.child_tasks.write_wire(writer)?;
        self.presentation.write_wire(writer)?;
        self.audio.write_wire(writer)
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        Ok(Self {
            child_tasks: AwbcChildCleanup::read_wire(reader)?,
            presentation: AwbcPresentationCleanup::read_wire(reader)?,
            audio: AwbcAudioCleanup::read_wire(reader)?,
        })
    }
}

wire_enum!(AwbcChildCleanup, "child cleanup", {
    0 => AwbcChildCleanup::CancelAndJoin,
    1 => AwbcChildCleanup::Detach,
    2 => AwbcChildCleanup::Finish,
});

wire_enum!(AwbcPresentationCleanup, "presentation cleanup", {
    0 => AwbcPresentationCleanup::DropRegistered,
    1 => AwbcPresentationCleanup::KeepRegistered,
});

wire_enum!(AwbcAudioCleanup, "audio cleanup", {
    0 => AwbcAudioCleanup::StopRegistered,
    1 => AwbcAudioCleanup::FadeRegistered,
    2 => AwbcAudioCleanup::KeepRegistered,
});

impl Wire for AwbcLineTaskNode {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        match self {
            Self::Sequence(nodes) => {
                writer.write_u8(0);
                nodes.write_wire(writer)?;
            }
            Self::Start(nodes) => {
                writer.write_u8(1);
                nodes.write_wire(writer)?;
            }
            Self::Parallel { policy, children } => {
                writer.write_u8(2);
                policy.write_wire(writer)?;
                children.write_wire(writer)?;
            }
            Self::Child {
                trigger,
                join,
                cancel,
                scope,
            } => {
                writer.write_u8(3);
                trigger.write_wire(writer)?;
                join.write_wire(writer)?;
                cancel.write_wire(writer)?;
                scope.write_wire(writer)?;
            }
            Self::Action(function) => {
                writer.write_u8(4);
                function.write_wire(writer)?;
            }
        }
        Ok(())
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        let offset = reader.offset();
        Ok(match reader.read_u8()? {
            0 => Self::Sequence(Vec::<AwbcLineTaskNodeId>::read_wire(reader)?),
            1 => Self::Start(Vec::<AwbcLineTaskNodeId>::read_wire(reader)?),
            2 => Self::Parallel {
                policy: AwbcParallelPolicy::read_wire(reader)?,
                children: Vec::<AwbcLineTaskNodeId>::read_wire(reader)?,
            },
            3 => Self::Child {
                trigger: AwbcLineTaskTrigger::read_wire(reader)?,
                join: AwbcChildJoinPolicy::read_wire(reader)?,
                cancel: AwbcChildCancelPolicy::read_wire(reader)?,
                scope: AwbcLineTaskNodeId::read_wire(reader)?,
            },
            4 => Self::Action(AwbcFunctionId::read_wire(reader)?),
            tag => {
                return Err(AwbcCodecError::UnknownTag {
                    kind: "line task node",
                    tag,
                    offset,
                });
            }
        })
    }
}

wire_enum!(AwbcParallelPolicy, "parallel policy", {
    0 => AwbcParallelPolicy::JoinAll,
});

impl Wire for AwbcLineTaskTrigger {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        match self {
            Self::Immediate => writer.write_u8(0),
            Self::Mark(mark) => {
                writer.write_u8(1);
                mark.get().get().write_wire(writer)?;
            }
            Self::Scheduled(site) => {
                writer.write_u8(2);
                site.write_wire(writer)?;
            }
            Self::ContentEffect(site) => {
                writer.write_u8(3);
                site.get().get().write_wire(writer)?;
            }
        }
        Ok(())
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        let offset = reader.offset();
        Ok(match reader.read_u8()? {
            0 => Self::Immediate,
            1 => Self::Mark(runtime_dialogue_mark_id(reader)?),
            2 => Self::Scheduled(AwbcLineHandleSiteId::read_wire(reader)?),
            3 => Self::ContentEffect(runtime_dialogue_effect_site_id(reader)?),
            tag => {
                return Err(AwbcCodecError::UnknownTag {
                    kind: "line task trigger",
                    tag,
                    offset,
                });
            }
        })
    }
}

fn runtime_dialogue_effect_site_id(
    reader: &mut Reader<'_>,
) -> Result<RuntimeDialogueEffectSiteId, AwbcCodecError> {
    NonZeroU32::new(u32::read_wire(reader)?)
        .map(RuntimeDialogueEffectSiteId::from_accepted_ordinal)
        .ok_or_else(|| AwbcCodecError::InvalidMetadata {
            kind: "runtime dialogue effect site identity",
            message: "must be nonzero".to_owned(),
            offset: reader.offset(),
        })
}

impl Wire for RuntimeLocalDeclarationId {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        self.get().get().write_wire(writer)
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        NonZeroU32::new(u32::read_wire(reader)?)
            .map(Self::from_accepted_ordinal)
            .ok_or_else(|| AwbcCodecError::InvalidMetadata {
                kind: "runtime local declaration identity",
                message: "must be nonzero".to_owned(),
                offset: reader.offset(),
            })
    }
}

pub(super) fn runtime_dialogue_mark_id(
    reader: &mut Reader<'_>,
) -> Result<RuntimeDialogueMarkId, AwbcCodecError> {
    NonZeroU32::new(u32::read_wire(reader)?)
        .map(RuntimeDialogueMarkId::from_accepted_ordinal)
        .ok_or_else(|| AwbcCodecError::InvalidMetadata {
            kind: "runtime dialogue mark identity",
            message: "must be nonzero".to_owned(),
            offset: reader.offset(),
        })
}

wire_enum!(AwbcChildJoinPolicy, "child join policy", {
    0 => AwbcChildJoinPolicy::Join,
    1 => AwbcChildJoinPolicy::Detached,
});

wire_enum!(AwbcChildCancelPolicy, "child cancel policy", {
    0 => AwbcChildCancelPolicy::CancelAndJoin,
    1 => AwbcChildCancelPolicy::Finish,
    2 => AwbcChildCancelPolicy::Detach,
});

impl Wire for AwbcStreamPlan {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        self.public_id.write_wire(writer)?;
        self.item_type.write_wire(writer)?;
        self.error_type.write_wire(writer)?;
        self.transform.write_wire(writer)
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        Ok(Self {
            public_id: AwbcStringId::read_wire(reader)?,
            item_type: AwbcTypeId::read_wire(reader)?,
            error_type: AwbcTypeId::read_wire(reader)?,
            transform: AwbcFunctionId::read_wire(reader)?,
        })
    }
}

impl Wire for AwbcPureHelper {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        self.public_id.write_wire(writer)?;
        self.signature.write_wire(writer)?;
        self.function.write_wire(writer)?;
        self.scalar_eval_supported.write_wire(writer)?;
        self.origin.write_wire(writer)
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        Ok(Self {
            public_id: AwbcStringId::read_wire(reader)?,
            signature: AwbcSignatureId::read_wire(reader)?,
            function: AwbcFunctionId::read_wire(reader)?,
            scalar_eval_supported: bool::read_wire(reader)?,
            origin: AwbcPureHelperOrigin::read_wire(reader)?,
        })
    }
}

wire_enum!(AwbcPureHelperOrigin, "pure helper origin", {
    0 => AwbcPureHelperOrigin::Annotated,
    1 => AwbcPureHelperOrigin::Inferred,
    2 => AwbcPureHelperOrigin::EngineOwned,
});

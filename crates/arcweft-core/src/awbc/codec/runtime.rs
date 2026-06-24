use super::AwbcCodecError;
use super::wire::{Reader, Wire, Writer, wire_enum};
use crate::awbc::schema::{
    AwbcAudioCleanup, AwbcAwaitManyPolicy, AwbcBackpressurePolicy, AwbcChildCancelPolicy,
    AwbcChildCleanup, AwbcChildJoinPolicy, AwbcChoice, AwbcChoiceOption, AwbcConflictPolicy,
    AwbcConstantId, AwbcEffectKind, AwbcEffectPlan, AwbcEffectPlanId, AwbcFunctionId, AwbcHostCall,
    AwbcHostCallMode, AwbcIntrinsic, AwbcLineCancelHandler, AwbcLineCleanupPolicy, AwbcLineOption,
    AwbcLineTaskGroup, AwbcLineTaskNode, AwbcLineTaskNodeId, AwbcLineTaskTrigger,
    AwbcOverflowPolicy, AwbcParallelPolicy, AwbcPresentationCleanup, AwbcPrivacyPolicy,
    AwbcPureHelper, AwbcPureHelperOrigin, AwbcReduceOp, AwbcRegisterId, AwbcReplayPolicy,
    AwbcResourceAccess, AwbcResourceAccessMode, AwbcResourceId, AwbcSignatureId,
    AwbcSourceEventKind, AwbcSourceHandler, AwbcSourcePlan, AwbcSourcePolicy, AwbcStreamPlan,
    AwbcStringId, AwbcTableRange, AwbcTaskArgument, AwbcTaskClass, AwbcTaskPlan, AwbcTaskPlanId,
    AwbcTaskPolicy, AwbcTypeId,
};

impl Wire for AwbcIntrinsic {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        self.public_id.write_wire(writer)?;
        self.registry_code.write_wire(writer)?;
        self.signature.write_wire(writer)?;
        self.revision.write_wire(writer)
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        Ok(Self {
            public_id: AwbcStringId::read_wire(reader)?,
            registry_code: u32::read_wire(reader)?,
            signature: AwbcSignatureId::read_wire(reader)?,
            revision: u32::read_wire(reader)?,
        })
    }
}

impl Wire for AwbcHostCall {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        self.public_id.write_wire(writer)?;
        self.capability.write_wire(writer)?;
        self.operation.write_wire(writer)?;
        self.signature.write_wire(writer)?;
        self.mode.write_wire(writer)?;
        self.deterministic.write_wire(writer)
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        Ok(Self {
            public_id: AwbcStringId::read_wire(reader)?,
            capability: AwbcStringId::read_wire(reader)?,
            operation: AwbcStringId::read_wire(reader)?,
            signature: AwbcSignatureId::read_wire(reader)?,
            mode: AwbcHostCallMode::read_wire(reader)?,
            deterministic: bool::read_wire(reader)?,
        })
    }
}

wire_enum!(AwbcHostCallMode, "host call mode", {
    0 => AwbcHostCallMode::Immediate,
    1 => AwbcHostCallMode::Suspend,
});

impl Wire for AwbcTaskPlan {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        self.public_id.write_wire(writer)?;
        self.capability.write_wire(writer)?;
        self.operation.write_wire(writer)?;
        self.signature.write_wire(writer)?;
        self.class.write_wire(writer)?;
        self.priority.write_wire(writer)?;
        self.cancel_scope.write_wire(writer)?;
        self.policy.write_wire(writer)?;
        self.arguments.write_wire(writer)?;
        self.many.write_wire(writer)
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        Ok(Self {
            public_id: AwbcStringId::read_wire(reader)?,
            capability: AwbcStringId::read_wire(reader)?,
            operation: AwbcStringId::read_wire(reader)?,
            signature: AwbcSignatureId::read_wire(reader)?,
            class: AwbcTaskClass::read_wire(reader)?,
            priority: i32::read_wire(reader)?,
            cancel_scope: AwbcStringId::read_wire(reader)?,
            policy: AwbcTaskPolicy::read_wire(reader)?,
            arguments: Vec::<AwbcTaskArgument>::read_wire(reader)?,
            many: Option::<AwbcAwaitManyPolicy>::read_wire(reader)?,
        })
    }
}

wire_enum!(AwbcTaskClass, "task class", {
    0 => AwbcTaskClass::LocalUi,
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

impl Wire for AwbcTaskArgument {
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

impl Wire for AwbcEffectPlan {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        self.kind.write_wire(writer)?;
        self.signature.write_wire(writer)?;
        self.capability.write_wire(writer)?;
        self.static_args.write_wire(writer)?;
        self.resources.write_wire(writer)
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        Ok(Self {
            kind: AwbcEffectKind::read_wire(reader)?,
            signature: AwbcSignatureId::read_wire(reader)?,
            capability: Option::<AwbcStringId>::read_wire(reader)?,
            static_args: Vec::<AwbcConstantId>::read_wire(reader)?,
            resources: Vec::<AwbcResourceAccess>::read_wire(reader)?,
        })
    }
}

wire_enum!(AwbcEffectKind, "effect kind", {
    0 => AwbcEffectKind::RegisterHandle,
    1 => AwbcEffectKind::DropHandle,
    2 => AwbcEffectKind::Wait,
    3 => AwbcEffectKind::Audio,
    4 => AwbcEffectKind::Call,
    5 => AwbcEffectKind::Log,
    6 => AwbcEffectKind::SignalWrite,
    7 => AwbcEffectKind::MetricWrite,
    8 => AwbcEffectKind::EmitEvent,
    9 => AwbcEffectKind::Out,
    10 => AwbcEffectKind::Return,
    11 => AwbcEffectKind::Goto,
    12 => AwbcEffectKind::Panic,
    13 => AwbcEffectKind::Fail,
    14 => AwbcEffectKind::Bail,
    15 => AwbcEffectKind::Ensure,
    16 => AwbcEffectKind::Assert,
    17 => AwbcEffectKind::Close,
    18 => AwbcEffectKind::Select,
    19 => AwbcEffectKind::Break,
    20 => AwbcEffectKind::Continue,
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
        self.root.write_wire(writer)?;
        self.options.write_wire(writer)?;
        self.bindings.write_wire(writer)?;
        self.out.write_wire(writer)?;
        self.cancel_handlers.write_wire(writer)?;
        self.cleanup.write_wire(writer)
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        Ok(Self {
            root: AwbcLineTaskNodeId::read_wire(reader)?,
            options: Vec::<AwbcLineOption>::read_wire(reader)?,
            bindings: Option::<AwbcFunctionId>::read_wire(reader)?,
            out: Option::<AwbcFunctionId>::read_wire(reader)?,
            cancel_handlers: Vec::<AwbcLineCancelHandler>::read_wire(reader)?,
            cleanup: AwbcLineCleanupPolicy::read_wire(reader)?,
        })
    }
}

impl Wire for AwbcLineOption {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        self.name.write_wire(writer)?;
        self.value.write_wire(writer)
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        Ok(Self {
            name: AwbcStringId::read_wire(reader)?,
            value: AwbcConstantId::read_wire(reader)?,
        })
    }
}

impl Wire for AwbcLineCancelHandler {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        self.trigger.write_wire(writer)?;
        self.function.write_wire(writer)
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        Ok(Self {
            trigger: AwbcStringId::read_wire(reader)?,
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
                task,
                trigger,
                join,
                cancel,
                scope,
            } => {
                writer.write_u8(3);
                task.write_wire(writer)?;
                trigger.write_wire(writer)?;
                join.write_wire(writer)?;
                cancel.write_wire(writer)?;
                scope.write_wire(writer)?;
            }
            Self::Effect(effect) => {
                writer.write_u8(4);
                effect.write_wire(writer)?;
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
                task: AwbcTaskPlanId::read_wire(reader)?,
                trigger: AwbcLineTaskTrigger::read_wire(reader)?,
                join: AwbcChildJoinPolicy::read_wire(reader)?,
                cancel: AwbcChildCancelPolicy::read_wire(reader)?,
                scope: AwbcLineTaskNodeId::read_wire(reader)?,
            },
            4 => Self::Effect(AwbcEffectPlanId::read_wire(reader)?),
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
                mark.write_wire(writer)?;
            }
            Self::DelayNanos(nanos) => {
                writer.write_u8(2);
                nanos.write_wire(writer)?;
            }
        }
        Ok(())
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        let offset = reader.offset();
        Ok(match reader.read_u8()? {
            0 => Self::Immediate,
            1 => Self::Mark(AwbcStringId::read_wire(reader)?),
            2 => Self::DelayNanos(u64::read_wire(reader)?),
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

impl Wire for AwbcSourcePlan {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        self.public_id.write_wire(writer)?;
        self.item_type.write_wire(writer)?;
        self.error_type.write_wire(writer)?;
        self.open.write_wire(writer)?;
        self.policy.write_wire(writer)?;
        self.handlers.write_wire(writer)
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        Ok(Self {
            public_id: AwbcStringId::read_wire(reader)?,
            item_type: AwbcTypeId::read_wire(reader)?,
            error_type: AwbcTypeId::read_wire(reader)?,
            open: AwbcFunctionId::read_wire(reader)?,
            policy: AwbcSourcePolicy::read_wire(reader)?,
            handlers: Vec::<AwbcSourceHandler>::read_wire(reader)?,
        })
    }
}

impl Wire for AwbcSourceHandler {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        self.kind.write_wire(writer)?;
        self.function.write_wire(writer)
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        Ok(Self {
            kind: AwbcSourceEventKind::read_wire(reader)?,
            function: AwbcFunctionId::read_wire(reader)?,
        })
    }
}

wire_enum!(AwbcSourceEventKind, "source event kind", {
    0 => AwbcSourceEventKind::Item,
    1 => AwbcSourceEventKind::Error,
    2 => AwbcSourceEventKind::Progress,
    3 => AwbcSourceEventKind::Disconnected,
    4 => AwbcSourceEventKind::PermissionRevoked,
    5 => AwbcSourceEventKind::End,
});

impl Wire for AwbcSourcePolicy {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        self.backpressure.write_wire(writer)?;
        self.replay.write_wire(writer)?;
        self.privacy.write_wire(writer)?;
        self.max_queue.write_wire(writer)
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        Ok(Self {
            backpressure: AwbcBackpressurePolicy::read_wire(reader)?,
            replay: AwbcReplayPolicy::read_wire(reader)?,
            privacy: AwbcPrivacyPolicy::read_wire(reader)?,
            max_queue: u32::read_wire(reader)?,
        })
    }
}

impl Wire for AwbcBackpressurePolicy {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        match self {
            Self::LatestOnly => writer.write_u8(0),
            Self::BoundedQueue { capacity, overflow } => {
                writer.write_u8(1);
                capacity.write_wire(writer)?;
                overflow.write_wire(writer)?;
            }
            Self::BlockingNotAllowed => writer.write_u8(2),
        }
        Ok(())
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        let offset = reader.offset();
        Ok(match reader.read_u8()? {
            0 => Self::LatestOnly,
            1 => Self::BoundedQueue {
                capacity: u32::read_wire(reader)?,
                overflow: AwbcOverflowPolicy::read_wire(reader)?,
            },
            2 => Self::BlockingNotAllowed,
            tag => {
                return Err(AwbcCodecError::UnknownTag {
                    kind: "backpressure policy",
                    tag,
                    offset,
                });
            }
        })
    }
}

wire_enum!(AwbcOverflowPolicy, "overflow policy", {
    0 => AwbcOverflowPolicy::DropOldest,
    1 => AwbcOverflowPolicy::DropNewest,
    2 => AwbcOverflowPolicy::Error,
    3 => AwbcOverflowPolicy::Coalesce,
});

wire_enum!(AwbcReplayPolicy, "replay policy", {
    0 => AwbcReplayPolicy::Full,
    1 => AwbcReplayPolicy::HashOnly,
    2 => AwbcReplayPolicy::Summary,
    3 => AwbcReplayPolicy::EventOnly,
    4 => AwbcReplayPolicy::None,
});

wire_enum!(AwbcPrivacyPolicy, "privacy policy", {
    0 => AwbcPrivacyPolicy::Transient,
    1 => AwbcPrivacyPolicy::Redacted,
    2 => AwbcPrivacyPolicy::Recordable,
    3 => AwbcPrivacyPolicy::Private,
});

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
});

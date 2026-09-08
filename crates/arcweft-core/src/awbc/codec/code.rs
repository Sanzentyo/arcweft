#![allow(
    clippy::too_many_lines,
    reason = "AWBC wire tables encode large tagged instruction families in one canonical order"
)]

use super::AwbcCodecError;
use super::wire::{Reader, Wire, Writer};
use crate::awbc::schema::{
    AwbcAwaitObserverResume, AwbcBinaryOp, AwbcBindMode, AwbcBlock, AwbcBlockId, AwbcChoiceId,
    AwbcConstantId, AwbcContentUnitId, AwbcDialogueContentEffectBinding, AwbcDialogueResultTarget,
    AwbcDialogueValueBinding, AwbcDialogueValueRole, AwbcDropPolicy, AwbcEffectPlanId,
    AwbcFieldProjection, AwbcFrameLayoutId, AwbcFunction, AwbcFunctionFlags, AwbcFunctionId,
    AwbcFunctionKind, AwbcHostCallId, AwbcInstruction, AwbcIntrinsicId, AwbcLineOperationId,
    AwbcMatchArm, AwbcOpcode, AwbcOpcodeClass, AwbcPattern, AwbcPatternId, AwbcPatternRest,
    AwbcProjectCall, AwbcProjectCallAttachedMaterialization, AwbcProjectCallAttachedPresence,
    AwbcProjectCallCaptureSource, AwbcProjectCallDefaultFunction, AwbcProjectCallInput,
    AwbcProjectCallOperand, AwbcProjectCallOperandMode, AwbcProjectCallOrdinaryMaterialization,
    AwbcProjectCallOutcome, AwbcProjectContinuationAbi, AwbcPureHelperId, AwbcRecordPatternField,
    AwbcRegisterId, AwbcResumePoint, AwbcResumePointId, AwbcSafePointKind, AwbcScopeId,
    AwbcSignatureId, AwbcSourceMapId, AwbcStreamPlanId, AwbcStringId, AwbcTableRange,
    AwbcTaskPlanId, AwbcTerminator, AwbcTraitMethodId, AwbcTrapCode, AwbcTypeId, AwbcUnaryOp,
};
use crate::value::RuntimeAgentConstructor;

impl Wire for AwbcFunction {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        self.public_id.write_wire(writer)?;
        self.kind.write_wire(writer)?;
        self.signature.write_wire(writer)?;
        self.frame_layout.write_wire(writer)?;
        self.blocks.write_wire(writer)?;
        self.entry_block.write_wire(writer)?;
        self.flags.write_wire(writer)
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        Ok(Self {
            public_id: Option::<AwbcStringId>::read_wire(reader)?,
            kind: AwbcFunctionKind::read_wire(reader)?,
            signature: AwbcSignatureId::read_wire(reader)?,
            frame_layout: AwbcFrameLayoutId::read_wire(reader)?,
            blocks: AwbcTableRange::read_wire(reader)?,
            entry_block: AwbcBlockId::read_wire(reader)?,
            flags: AwbcFunctionFlags::read_wire(reader)?,
        })
    }
}

impl Wire for AwbcFunctionKind {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        writer.write_u8(self.encoded());
        Ok(())
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        let offset = reader.offset();
        let tag = reader.read_u8()?;
        Self::from_encoded(tag).ok_or(AwbcCodecError::UnknownTag {
            kind: "function kind",
            tag,
            offset,
        })
    }
}

impl Wire for AwbcDialogueValueRole {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        writer.write_u8(self.encoded());
        Ok(())
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        let offset = reader.offset();
        let tag = reader.read_u8()?;
        Self::from_encoded(tag).ok_or(AwbcCodecError::UnknownTag {
            kind: "dialogue value role",
            tag,
            offset,
        })
    }
}

impl Wire for AwbcDialogueValueBinding {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        self.slot.get().get().write_wire(writer)?;
        self.role.write_wire(writer)?;
        self.value.write_wire(writer)
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        let offset = reader.offset();
        let encoded = u32::read_wire(reader)?;
        let slot = encoded
            .checked_sub(1)
            .and_then(|index| {
                crate::runtime_id::RuntimeDialogueValueSlotId::from_zero_based(index as usize)
            })
            .ok_or_else(|| AwbcCodecError::InvalidMetadata {
                kind: "dialogue value slot",
                message: "slot identity must be nonzero".to_owned(),
                offset,
            })?;
        Ok(Self {
            slot,
            role: AwbcDialogueValueRole::read_wire(reader)?,
            value: AwbcRegisterId::read_wire(reader)?,
        })
    }
}

impl Wire for AwbcDialogueContentEffectBinding {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        self.site.get().get().write_wire(writer)?;
        self.function.write_wire(writer)?;
        self.captures.write_wire(writer)
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        let offset = reader.offset();
        let site = u32::read_wire(reader)?;
        let site = std::num::NonZeroU32::new(site)
            .map(crate::runtime_id::RuntimeDialogueEffectSiteId::from_accepted_ordinal)
            .ok_or_else(|| AwbcCodecError::InvalidMetadata {
                kind: "dialogue content effect site",
                message: "effect-site identity must be nonzero".to_owned(),
                offset,
            })?;
        Ok(Self {
            site,
            function: AwbcFunctionId::read_wire(reader)?,
            captures: Vec::<AwbcRegisterId>::read_wire(reader)?,
        })
    }
}

impl Wire for AwbcDialogueResultTarget {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        self.ty.write_wire(writer)?;
        self.pattern.write_wire(writer)?;
        self.destination.write_wire(writer)
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        Ok(Self {
            ty: AwbcTypeId::read_wire(reader)?,
            pattern: AwbcPatternId::read_wire(reader)?,
            destination: AwbcRegisterId::read_wire(reader)?,
        })
    }
}

impl Wire for AwbcBlock {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        self.owner.write_wire(writer)?;
        self.instructions.write_wire(writer)?;
        self.terminator.write_wire(writer)?;
        self.safe_point.write_wire(writer)?;
        self.source_map.write_wire(writer)
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        Ok(Self {
            owner: AwbcFunctionId::read_wire(reader)?,
            instructions: AwbcTableRange::read_wire(reader)?,
            terminator: AwbcTerminator::read_wire(reader)?,
            safe_point: AwbcSafePointKind::read_wire(reader)?,
            source_map: Option::<AwbcSourceMapId>::read_wire(reader)?,
        })
    }
}

impl Wire for AwbcFieldProjection {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        match self {
            Self::Named(field) => {
                writer.write_u8(0);
                field.write_wire(writer)?;
            }
            Self::OpaqueRecord {
                owner,
                field,
                field_type,
            } => {
                writer.write_u8(1);
                owner.write_wire(writer)?;
                field.write_wire(writer)?;
                field_type.write_wire(writer)?;
            }
        }
        Ok(())
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        let offset = reader.offset();
        Ok(match reader.read_u8()? {
            0 => Self::Named(AwbcStringId::read_wire(reader)?),
            1 => Self::OpaqueRecord {
                owner: AwbcTypeId::read_wire(reader)?,
                field: u32::read_wire(reader)?,
                field_type: AwbcTypeId::read_wire(reader)?,
            },
            tag => {
                return Err(AwbcCodecError::UnknownTag {
                    kind: "field projection",
                    tag,
                    offset,
                });
            }
        })
    }
}

impl Wire for AwbcInstruction {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        writer.write_u8(self.opcode().encoded());
        match self {
            Self::Nop => {}
            Self::LoadConst { dst, constant } => {
                dst.write_wire(writer)?;
                constant.write_wire(writer)?;
            }
            Self::Move { dst, src } | Self::CopyValue { dst, src } => {
                dst.write_wire(writer)?;
                src.write_wire(writer)?;
            }
            Self::Clear { register } => register.write_wire(writer)?,
            Self::Drop { register, policy } => {
                register.write_wire(writer)?;
                policy.write_wire(writer)?;
            }
            Self::EnterScope { scope } | Self::ExitScope { scope } => scope.write_wire(writer)?,
            Self::BindPattern {
                pattern,
                value,
                mode,
            } => {
                pattern.write_wire(writer)?;
                value.write_wire(writer)?;
                mode.write_wire(writer)?;
            }
            Self::TestPattern {
                dst,
                pattern,
                value,
            } => {
                dst.write_wire(writer)?;
                pattern.write_wire(writer)?;
                value.write_wire(writer)?;
            }
            Self::MakeTuple { dst, items } | Self::MakeSequence { dst, items } => {
                dst.write_wire(writer)?;
                items.write_wire(writer)?;
            }
            Self::RepeatSequence { dst, value, len } => {
                dst.write_wire(writer)?;
                value.write_wire(writer)?;
                len.write_wire(writer)?;
            }
            Self::SequenceLen { dst, sequence } => {
                dst.write_wire(writer)?;
                sequence.write_wire(writer)?;
            }
            Self::SequenceGet {
                dst,
                sequence,
                index,
            } => {
                dst.write_wire(writer)?;
                sequence.write_wire(writer)?;
                index.write_wire(writer)?;
            }
            Self::SequenceSlice {
                dst,
                sequence,
                start,
            } => {
                dst.write_wire(writer)?;
                sequence.write_wire(writer)?;
                start.write_wire(writer)?;
            }
            Self::SequencePush { sequence, value } => {
                sequence.write_wire(writer)?;
                value.write_wire(writer)?;
            }
            Self::MakeRecord {
                dst,
                ty,
                field_names,
                fields,
            } => {
                dst.write_wire(writer)?;
                ty.write_wire(writer)?;
                field_names.write_wire(writer)?;
                fields.write_wire(writer)?;
            }
            Self::MakeVariant {
                dst,
                ty,
                case,
                case_name,
                payload,
            } => {
                dst.write_wire(writer)?;
                ty.write_wire(writer)?;
                case.write_wire(writer)?;
                case_name.write_wire(writer)?;
                payload.write_wire(writer)?;
            }
            Self::ProjectTuple {
                dst,
                target,
                ordinal,
            }
            | Self::ProjectRecord {
                dst,
                target,
                ordinal,
            } => {
                dst.write_wire(writer)?;
                target.write_wire(writer)?;
                ordinal.write_wire(writer)?;
            }
            Self::ProjectField { dst, target, field } => {
                dst.write_wire(writer)?;
                target.write_wire(writer)?;
                field.write_wire(writer)?;
            }
            Self::Unary { dst, op, src } => {
                dst.write_wire(writer)?;
                op.write_wire(writer)?;
                src.write_wire(writer)?;
            }
            Self::Binary { dst, op, lhs, rhs } => {
                dst.write_wire(writer)?;
                op.write_wire(writer)?;
                lhs.write_wire(writer)?;
                rhs.write_wire(writer)?;
            }
            Self::CallPureHelper { dst, helper, args } => {
                dst.write_wire(writer)?;
                helper.write_wire(writer)?;
                args.write_wire(writer)?;
            }
            Self::CallIntrinsic {
                dst,
                intrinsic,
                args,
            } => {
                dst.write_wire(writer)?;
                intrinsic.write_wire(writer)?;
                args.write_wire(writer)?;
            }
            Self::EnsureContent { content } => content.write_wire(writer)?,
            Self::MakeDialogueContent {
                destination,
                template,
                values,
                effects,
            } => {
                destination.write_wire(writer)?;
                template.write_wire(writer)?;
                values.write_wire(writer)?;
                effects.write_wire(writer)?;
            }
            Self::EmitEffect { effect, args } => {
                effect.write_wire(writer)?;
                args.write_wire(writer)?;
            }
            Self::StartTask { dst, plan, args } => {
                dst.write_wire(writer)?;
                plan.write_wire(writer)?;
                args.write_wire(writer)?;
            }
            Self::SpawnFiber {
                dst,
                function,
                args,
            } => {
                dst.write_wire(writer)?;
                function.write_wire(writer)?;
                args.write_wire(writer)?;
            }
            Self::StreamYield { stream, value } => {
                stream.write_wire(writer)?;
                value.write_wire(writer)?;
            }
            Self::StreamClose { stream } => stream.write_wire(writer)?,
            Self::ExecuteLineOperation {
                dst,
                operation,
                args,
            } => {
                dst.write_wire(writer)?;
                operation.write_wire(writer)?;
                args.write_wire(writer)?;
            }
            Self::CommitDialogueResult { source } => source.write_wire(writer)?,
            Self::AssignRecordField {
                target,
                field,
                value,
            } => {
                target.write_wire(writer)?;
                field.write_wire(writer)?;
                value.write_wire(writer)?;
            }
            Self::CallTraitMethod {
                dst,
                method,
                receiver,
                args,
                receiver_out,
            } => {
                dst.write_wire(writer)?;
                method.write_wire(writer)?;
                receiver.write_wire(writer)?;
                args.write_wire(writer)?;
                receiver_out.write_wire(writer)?;
            }
            Self::RegisterCleanup { key, effect, args } => {
                key.write_wire(writer)?;
                effect.write_wire(writer)?;
                args.write_wire(writer)?;
            }
            Self::CancelCleanup { key } => key.write_wire(writer)?,
            Self::MakeFunction {
                dst,
                function,
                params,
                capture_names,
                captures,
            } => {
                dst.write_wire(writer)?;
                function.write_wire(writer)?;
                params.write_wire(writer)?;
                capture_names.write_wire(writer)?;
                captures.write_wire(writer)?;
            }
            Self::ApplyFunction { dst, callee, args } => {
                dst.write_wire(writer)?;
                callee.write_wire(writer)?;
                args.write_wire(writer)?;
            }
            Self::MakeAgent {
                dst,
                constructor,
                operands,
            } => {
                dst.write_wire(writer)?;
                constructor.write_wire(writer)?;
                operands.write_wire(writer)?;
            }
            Self::MakeReductionUnchanged { dst, ty, state } => {
                dst.write_wire(writer)?;
                ty.write_wire(writer)?;
                state.write_wire(writer)?;
            }
        }
        Ok(())
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        let offset = reader.offset();
        let encoded = reader.read_u8()?;
        let Some(opcode) = AwbcOpcode::from_encoded(encoded) else {
            return Err(AwbcCodecError::UnknownTag {
                kind: "instruction opcode",
                tag: encoded,
                offset,
            });
        };
        if opcode.class() != AwbcOpcodeClass::Instruction {
            return Err(AwbcCodecError::UnknownTag {
                kind: "instruction opcode",
                tag: encoded,
                offset,
            });
        }
        Ok(match opcode {
            AwbcOpcode::Nop => Self::Nop,
            AwbcOpcode::LoadConst => Self::LoadConst {
                dst: AwbcRegisterId::read_wire(reader)?,
                constant: AwbcConstantId::read_wire(reader)?,
            },
            AwbcOpcode::Move => Self::Move {
                dst: AwbcRegisterId::read_wire(reader)?,
                src: AwbcRegisterId::read_wire(reader)?,
            },
            AwbcOpcode::CopyValue => Self::CopyValue {
                dst: AwbcRegisterId::read_wire(reader)?,
                src: AwbcRegisterId::read_wire(reader)?,
            },
            AwbcOpcode::Clear => Self::Clear {
                register: AwbcRegisterId::read_wire(reader)?,
            },
            AwbcOpcode::EnterScope => Self::EnterScope {
                scope: AwbcScopeId::read_wire(reader)?,
            },
            AwbcOpcode::ExitScope => Self::ExitScope {
                scope: AwbcScopeId::read_wire(reader)?,
            },
            AwbcOpcode::BindPattern => Self::BindPattern {
                pattern: AwbcPatternId::read_wire(reader)?,
                value: AwbcRegisterId::read_wire(reader)?,
                mode: AwbcBindMode::read_wire(reader)?,
            },
            AwbcOpcode::TestPattern => Self::TestPattern {
                dst: AwbcRegisterId::read_wire(reader)?,
                pattern: AwbcPatternId::read_wire(reader)?,
                value: AwbcRegisterId::read_wire(reader)?,
            },
            AwbcOpcode::MakeTuple => Self::MakeTuple {
                dst: AwbcRegisterId::read_wire(reader)?,
                items: Vec::<AwbcRegisterId>::read_wire(reader)?,
            },
            AwbcOpcode::MakeSequence => Self::MakeSequence {
                dst: AwbcRegisterId::read_wire(reader)?,
                items: Vec::<AwbcRegisterId>::read_wire(reader)?,
            },
            AwbcOpcode::RepeatSequence => Self::RepeatSequence {
                dst: AwbcRegisterId::read_wire(reader)?,
                value: AwbcRegisterId::read_wire(reader)?,
                len: AwbcRegisterId::read_wire(reader)?,
            },
            AwbcOpcode::SequenceLen => Self::SequenceLen {
                dst: AwbcRegisterId::read_wire(reader)?,
                sequence: AwbcRegisterId::read_wire(reader)?,
            },
            AwbcOpcode::SequenceGet => Self::SequenceGet {
                dst: AwbcRegisterId::read_wire(reader)?,
                sequence: AwbcRegisterId::read_wire(reader)?,
                index: AwbcRegisterId::read_wire(reader)?,
            },
            AwbcOpcode::SequenceSlice => Self::SequenceSlice {
                dst: AwbcRegisterId::read_wire(reader)?,
                sequence: AwbcRegisterId::read_wire(reader)?,
                start: AwbcRegisterId::read_wire(reader)?,
            },
            AwbcOpcode::SequencePush => Self::SequencePush {
                sequence: AwbcRegisterId::read_wire(reader)?,
                value: AwbcRegisterId::read_wire(reader)?,
            },
            AwbcOpcode::MakeRecord => Self::MakeRecord {
                dst: AwbcRegisterId::read_wire(reader)?,
                ty: AwbcTypeId::read_wire(reader)?,
                field_names: Vec::<AwbcStringId>::read_wire(reader)?,
                fields: Vec::<AwbcRegisterId>::read_wire(reader)?,
            },
            AwbcOpcode::MakeVariant => Self::MakeVariant {
                dst: AwbcRegisterId::read_wire(reader)?,
                ty: AwbcTypeId::read_wire(reader)?,
                case: u32::read_wire(reader)?,
                case_name: AwbcStringId::read_wire(reader)?,
                payload: Option::<AwbcRegisterId>::read_wire(reader)?,
            },
            AwbcOpcode::ProjectTuple => Self::ProjectTuple {
                dst: AwbcRegisterId::read_wire(reader)?,
                target: AwbcRegisterId::read_wire(reader)?,
                ordinal: u32::read_wire(reader)?,
            },
            AwbcOpcode::ProjectRecord => Self::ProjectRecord {
                dst: AwbcRegisterId::read_wire(reader)?,
                target: AwbcRegisterId::read_wire(reader)?,
                ordinal: u32::read_wire(reader)?,
            },
            AwbcOpcode::ProjectField => Self::ProjectField {
                dst: AwbcRegisterId::read_wire(reader)?,
                target: AwbcRegisterId::read_wire(reader)?,
                field: AwbcFieldProjection::read_wire(reader)?,
            },
            AwbcOpcode::Unary => Self::Unary {
                dst: AwbcRegisterId::read_wire(reader)?,
                op: AwbcUnaryOp::read_wire(reader)?,
                src: AwbcRegisterId::read_wire(reader)?,
            },
            AwbcOpcode::Binary => Self::Binary {
                dst: AwbcRegisterId::read_wire(reader)?,
                op: AwbcBinaryOp::read_wire(reader)?,
                lhs: AwbcRegisterId::read_wire(reader)?,
                rhs: AwbcRegisterId::read_wire(reader)?,
            },
            AwbcOpcode::CallPureHelper => Self::CallPureHelper {
                dst: AwbcRegisterId::read_wire(reader)?,
                helper: AwbcPureHelperId::read_wire(reader)?,
                args: Vec::<AwbcRegisterId>::read_wire(reader)?,
            },
            AwbcOpcode::CallIntrinsic => Self::CallIntrinsic {
                dst: Option::<AwbcRegisterId>::read_wire(reader)?,
                intrinsic: AwbcIntrinsicId::read_wire(reader)?,
                args: Vec::<AwbcRegisterId>::read_wire(reader)?,
            },
            AwbcOpcode::EnsureContent => Self::EnsureContent {
                content: AwbcContentUnitId::read_wire(reader)?,
            },
            AwbcOpcode::MakeDialogueContent => Self::MakeDialogueContent {
                destination: AwbcRegisterId::read_wire(reader)?,
                template: crate::runtime_id::RuntimeDialogueContentTemplateId::read_wire(reader)?,
                values: Vec::<AwbcDialogueValueBinding>::read_wire(reader)?,
                effects: Vec::<AwbcDialogueContentEffectBinding>::read_wire(reader)?,
            },
            AwbcOpcode::EmitEffect => Self::EmitEffect {
                effect: AwbcEffectPlanId::read_wire(reader)?,
                args: Vec::<AwbcRegisterId>::read_wire(reader)?,
            },
            AwbcOpcode::StartTask => Self::StartTask {
                dst: AwbcRegisterId::read_wire(reader)?,
                plan: AwbcTaskPlanId::read_wire(reader)?,
                args: Vec::<AwbcRegisterId>::read_wire(reader)?,
            },
            AwbcOpcode::SpawnFiber => Self::SpawnFiber {
                dst: Option::<AwbcRegisterId>::read_wire(reader)?,
                function: AwbcFunctionId::read_wire(reader)?,
                args: Vec::<AwbcRegisterId>::read_wire(reader)?,
            },
            AwbcOpcode::StreamYield => Self::StreamYield {
                stream: AwbcStreamPlanId::read_wire(reader)?,
                value: AwbcRegisterId::read_wire(reader)?,
            },
            AwbcOpcode::StreamClose => Self::StreamClose {
                stream: AwbcStreamPlanId::read_wire(reader)?,
            },
            AwbcOpcode::ExecuteLineOperation => Self::ExecuteLineOperation {
                dst: AwbcRegisterId::read_wire(reader)?,
                operation: AwbcLineOperationId::read_wire(reader)?,
                args: Vec::<AwbcRegisterId>::read_wire(reader)?,
            },
            AwbcOpcode::CommitDialogueResult => Self::CommitDialogueResult {
                source: AwbcRegisterId::read_wire(reader)?,
            },
            AwbcOpcode::Drop => Self::Drop {
                register: AwbcRegisterId::read_wire(reader)?,
                policy: AwbcDropPolicy::read_wire(reader)?,
            },
            AwbcOpcode::AssignRecordField => Self::AssignRecordField {
                target: AwbcRegisterId::read_wire(reader)?,
                field: u32::read_wire(reader)?,
                value: AwbcRegisterId::read_wire(reader)?,
            },
            AwbcOpcode::CallTraitMethod => Self::CallTraitMethod {
                dst: AwbcRegisterId::read_wire(reader)?,
                method: AwbcTraitMethodId::read_wire(reader)?,
                receiver: AwbcRegisterId::read_wire(reader)?,
                args: Vec::<AwbcRegisterId>::read_wire(reader)?,
                receiver_out: Option::<AwbcRegisterId>::read_wire(reader)?,
            },
            AwbcOpcode::RegisterCleanup => Self::RegisterCleanup {
                key: AwbcStringId::read_wire(reader)?,
                effect: AwbcEffectPlanId::read_wire(reader)?,
                args: Vec::<AwbcRegisterId>::read_wire(reader)?,
            },
            AwbcOpcode::CancelCleanup => Self::CancelCleanup {
                key: AwbcStringId::read_wire(reader)?,
            },
            AwbcOpcode::MakeFunction => Self::MakeFunction {
                dst: AwbcRegisterId::read_wire(reader)?,
                function: AwbcFunctionId::read_wire(reader)?,
                params: Vec::<AwbcStringId>::read_wire(reader)?,
                capture_names: Vec::<AwbcStringId>::read_wire(reader)?,
                captures: Vec::<AwbcRegisterId>::read_wire(reader)?,
            },
            AwbcOpcode::ApplyFunction => Self::ApplyFunction {
                dst: AwbcRegisterId::read_wire(reader)?,
                callee: AwbcRegisterId::read_wire(reader)?,
                args: Vec::<AwbcRegisterId>::read_wire(reader)?,
            },
            AwbcOpcode::MakeAgent => Self::MakeAgent {
                dst: AwbcRegisterId::read_wire(reader)?,
                constructor: RuntimeAgentConstructor::read_wire(reader)?,
                operands: Vec::<AwbcRegisterId>::read_wire(reader)?,
            },
            AwbcOpcode::MakeReductionUnchanged => Self::MakeReductionUnchanged {
                dst: AwbcRegisterId::read_wire(reader)?,
                ty: AwbcTypeId::read_wire(reader)?,
                state: AwbcRegisterId::read_wire(reader)?,
            },
            AwbcOpcode::Jump
            | AwbcOpcode::Branch
            | AwbcOpcode::Match
            | AwbcOpcode::CallFunction
            | AwbcOpcode::GotoStatic
            | AwbcOpcode::GotoDynamic
            | AwbcOpcode::Dialogue
            | AwbcOpcode::Choice
            | AwbcOpcode::Await
            | AwbcOpcode::AwaitMany
            | AwbcOpcode::HostCall
            | AwbcOpcode::Return
            | AwbcOpcode::ProjectCall
            | AwbcOpcode::Trap
            | AwbcOpcode::BudgetYield
            | AwbcOpcode::Unreachable => unreachable!("terminator opcode rejected above"),
        })
    }
}

impl Wire for AwbcDropPolicy {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        match self {
            Self::Default => writer.write_u8(0),
            Self::Cancel => writer.write_u8(1),
            Self::Stop { fade } => {
                writer.write_u8(2);
                fade.write_wire(writer)?;
            }
            Self::Finish => writer.write_u8(3),
            Self::Release => writer.write_u8(4),
            Self::Detach => writer.write_u8(5),
        }
        Ok(())
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        let offset = reader.offset();
        Ok(match reader.read_u8()? {
            0 => Self::Default,
            1 => Self::Cancel,
            2 => Self::Stop {
                fade: AwbcRegisterId::read_wire(reader)?,
            },
            3 => Self::Finish,
            4 => Self::Release,
            5 => Self::Detach,
            tag => {
                return Err(AwbcCodecError::UnknownTag {
                    kind: "drop policy",
                    tag,
                    offset,
                });
            }
        })
    }
}

impl Wire for AwbcProjectCallOperandMode {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        writer.write_u8(self.encoded());
        Ok(())
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        let offset = reader.offset();
        let tag = reader.read_u8()?;
        Self::from_encoded(tag).ok_or(AwbcCodecError::UnknownTag {
            kind: "project-call operand mode",
            tag,
            offset,
        })
    }
}

impl Wire for arcweft_id::runtime_program::RuntimeProjectContinuationLineageId {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        self.as_bytes().write_wire(writer)
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        <[u8; 32]>::read_wire(reader).map(Self::from_checked_digest)
    }
}

impl Wire for AwbcProjectContinuationAbi {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        self.lineage.write_wire(writer)?;
        self.function_type.write_wire(writer)?;
        self.prefix_types.write_wire(writer)
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        Ok(Self {
            lineage: arcweft_id::runtime_program::RuntimeProjectContinuationLineageId::read_wire(
                reader,
            )?,
            function_type: AwbcTypeId::read_wire(reader)?,
            prefix_types: Vec::<AwbcTypeId>::read_wire(reader)?,
        })
    }
}

impl Wire for AwbcProjectCallOperand {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        self.value.write_wire(writer)?;
        self.mode.write_wire(writer)
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        Ok(Self {
            value: AwbcRegisterId::read_wire(reader)?,
            mode: AwbcProjectCallOperandMode::read_wire(reader)?,
        })
    }
}

impl Wire for AwbcProjectCallCaptureSource {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        match self {
            Self::ContinuationPrefix { position } => {
                writer.write_u8(0);
                position.write_wire(writer)?;
            }
            Self::CurrentLogical { position } => {
                writer.write_u8(1);
                position.write_wire(writer)?;
            }
        }
        Ok(())
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        let offset = reader.offset();
        Ok(match reader.read_u8()? {
            0 => Self::ContinuationPrefix {
                position: u32::read_wire(reader)?,
            },
            1 => Self::CurrentLogical {
                position: u32::read_wire(reader)?,
            },
            tag => {
                return Err(AwbcCodecError::UnknownTag {
                    kind: "project-call capture source",
                    tag,
                    offset,
                });
            }
        })
    }
}

impl Wire for AwbcProjectCallDefaultFunction {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        self.site.write_wire(writer)?;
        self.captures.write_wire(writer)
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        Ok(Self {
            site: AwbcFunctionId::read_wire(reader)?,
            captures: Vec::<AwbcProjectCallCaptureSource>::read_wire(reader)?,
        })
    }
}

impl Wire for AwbcProjectCallAttachedPresence {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        match self {
            Self::RequiredPresent => writer.write_u8(0),
            Self::OptionalPresent => writer.write_u8(1),
            Self::OptionalOmitted => writer.write_u8(2),
            Self::DefaultedPresent => writer.write_u8(3),
            Self::DefaultedOmitted { default } => {
                writer.write_u8(4);
                default.write_wire(writer)?;
            }
        }
        Ok(())
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        let offset = reader.offset();
        Ok(match reader.read_u8()? {
            0 => Self::RequiredPresent,
            1 => Self::OptionalPresent,
            2 => Self::OptionalOmitted,
            3 => Self::DefaultedPresent,
            4 => Self::DefaultedOmitted {
                default: AwbcProjectCallDefaultFunction::read_wire(reader)?,
            },
            tag => {
                return Err(AwbcCodecError::UnknownTag {
                    kind: "project-call attached presence",
                    tag,
                    offset,
                });
            }
        })
    }
}

impl Wire for AwbcProjectCallOrdinaryMaterialization {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        match self {
            Self::Fixed {
                parameter,
                abi_ty,
                binding_ty,
                source_index,
            } => {
                writer.write_u8(0);
                parameter.write_wire(writer)?;
                abi_ty.write_wire(writer)?;
                binding_ty.write_wire(writer)?;
                source_index.write_wire(writer)?;
            }
            Self::Rest {
                parameter,
                abi_ty,
                binding_ty,
                source_indices,
            } => {
                writer.write_u8(1);
                parameter.write_wire(writer)?;
                abi_ty.write_wire(writer)?;
                binding_ty.write_wire(writer)?;
                source_indices.write_wire(writer)?;
            }
        }
        Ok(())
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        let offset = reader.offset();
        Ok(match reader.read_u8()? {
            0 => Self::Fixed {
                parameter: u32::read_wire(reader)?,
                abi_ty: AwbcTypeId::read_wire(reader)?,
                binding_ty: AwbcTypeId::read_wire(reader)?,
                source_index: u32::read_wire(reader)?,
            },
            1 => Self::Rest {
                parameter: u32::read_wire(reader)?,
                abi_ty: AwbcTypeId::read_wire(reader)?,
                binding_ty: AwbcTypeId::read_wire(reader)?,
                source_indices: Vec::<u32>::read_wire(reader)?,
            },
            tag => {
                return Err(AwbcCodecError::UnknownTag {
                    kind: "project-call ordinary materialization",
                    tag,
                    offset,
                });
            }
        })
    }
}

impl Wire for AwbcProjectCallAttachedMaterialization {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        self.abi_ty.write_wire(writer)?;
        self.binding_ty.write_wire(writer)?;
        self.source_index.write_wire(writer)?;
        self.presence.write_wire(writer)
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        Ok(Self {
            abi_ty: AwbcTypeId::read_wire(reader)?,
            binding_ty: AwbcTypeId::read_wire(reader)?,
            source_index: Option::<u32>::read_wire(reader)?,
            presence: AwbcProjectCallAttachedPresence::read_wire(reader)?,
        })
    }
}

impl Wire for AwbcProjectCallInput {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        match self {
            Self::Direct => writer.write_u8(0),
            Self::Continuation {
                callee,
                expected_abi,
            } => {
                writer.write_u8(1);
                callee.write_wire(writer)?;
                expected_abi.write_wire(writer)?;
            }
        }
        Ok(())
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        let offset = reader.offset();
        Ok(match reader.read_u8()? {
            0 => Self::Direct,
            1 => Self::Continuation {
                callee: AwbcRegisterId::read_wire(reader)?,
                expected_abi: AwbcProjectContinuationAbi::read_wire(reader)?,
            },
            tag => {
                return Err(AwbcCodecError::UnknownTag {
                    kind: "project-call input",
                    tag,
                    offset,
                });
            }
        })
    }
}

impl Wire for AwbcProjectCallOutcome {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        match self {
            Self::Continue {
                result_abi,
                next_group,
            } => {
                writer.write_u8(0);
                result_abi.write_wire(writer)?;
                next_group.write_wire(writer)?;
            }
            Self::Invoke { function } => {
                writer.write_u8(1);
                function.write_wire(writer)?;
            }
        }
        Ok(())
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        let offset = reader.offset();
        Ok(match reader.read_u8()? {
            0 => Self::Continue {
                result_abi: AwbcProjectContinuationAbi::read_wire(reader)?,
                next_group: u32::read_wire(reader)?,
            },
            1 => Self::Invoke {
                function: AwbcFunctionId::read_wire(reader)?,
            },
            tag => {
                return Err(AwbcCodecError::UnknownTag {
                    kind: "project-call outcome",
                    tag,
                    offset,
                });
            }
        })
    }
}

impl Wire for AwbcProjectCall {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        self.input.write_wire(writer)?;
        self.completed_group.write_wire(writer)?;
        self.operands.write_wire(writer)?;
        self.ordinary.write_wire(writer)?;
        self.attached.write_wire(writer)?;
        self.outcome.write_wire(writer)?;
        self.result_ty.write_wire(writer)?;
        self.result_pattern.write_wire(writer)?;
        self.resume.write_wire(writer)
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        Ok(Self {
            input: AwbcProjectCallInput::read_wire(reader)?,
            completed_group: u32::read_wire(reader)?,
            operands: Vec::<AwbcProjectCallOperand>::read_wire(reader)?,
            ordinary: Vec::<AwbcProjectCallOrdinaryMaterialization>::read_wire(reader)?,
            attached: Option::<AwbcProjectCallAttachedMaterialization>::read_wire(reader)?,
            outcome: AwbcProjectCallOutcome::read_wire(reader)?,
            result_ty: AwbcTypeId::read_wire(reader)?,
            result_pattern: AwbcPatternId::read_wire(reader)?,
            resume: AwbcResumePointId::read_wire(reader)?,
        })
    }
}

impl Wire for AwbcTerminator {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        writer.write_u8(self.opcode().encoded());
        match self {
            Self::Jump { target } => target.write_wire(writer)?,
            Self::Branch {
                condition,
                then_block,
                else_block,
            } => {
                condition.write_wire(writer)?;
                then_block.write_wire(writer)?;
                else_block.write_wire(writer)?;
            }
            Self::Match {
                scrutinee,
                arms,
                default,
            } => {
                scrutinee.write_wire(writer)?;
                arms.write_wire(writer)?;
                default.write_wire(writer)?;
            }
            Self::CallFunction {
                function,
                args,
                dst,
                resume,
            } => {
                function.write_wire(writer)?;
                args.write_wire(writer)?;
                dst.write_wire(writer)?;
                resume.write_wire(writer)?;
            }
            Self::GotoStatic { function, args } => {
                function.write_wire(writer)?;
                args.write_wire(writer)?;
            }
            Self::GotoDynamic { target, args } => {
                target.write_wire(writer)?;
                args.write_wire(writer)?;
            }
            Self::Dialogue {
                content,
                values,
                effects,
                line_task_captures,
                result,
                resume,
            } => {
                content.write_wire(writer)?;
                values.write_wire(writer)?;
                effects.write_wire(writer)?;
                line_task_captures.write_wire(writer)?;
                result.write_wire(writer)?;
                resume.write_wire(writer)?;
            }
            Self::Choice {
                choice,
                dst,
                resume,
            } => {
                choice.write_wire(writer)?;
                dst.write_wire(writer)?;
                resume.write_wire(writer)?;
            }
            Self::Await {
                handle,
                binding,
                observer,
                resume,
            } => {
                handle.write_wire(writer)?;
                binding.write_wire(writer)?;
                observer.write_wire(writer)?;
                resume.write_wire(writer)?;
            }
            Self::AwaitMany {
                plan,
                source,
                binding,
                resume,
            } => {
                plan.write_wire(writer)?;
                source.write_wire(writer)?;
                binding.write_wire(writer)?;
                resume.write_wire(writer)?;
            }
            Self::HostCall {
                call,
                args,
                dst,
                resume,
            } => {
                call.write_wire(writer)?;
                args.write_wire(writer)?;
                dst.write_wire(writer)?;
                resume.write_wire(writer)?;
            }
            Self::ProjectCall { call } => call.write_wire(writer)?,
            Self::Return { value } => value.write_wire(writer)?,
            Self::Trap { code, message } => {
                code.write_wire(writer)?;
                message.write_wire(writer)?;
            }
            Self::BudgetYield { resume } => resume.write_wire(writer)?,
            Self::Unreachable => {}
        }
        Ok(())
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        let offset = reader.offset();
        let encoded = reader.read_u8()?;
        let Some(opcode) = AwbcOpcode::from_encoded(encoded) else {
            return Err(AwbcCodecError::UnknownTag {
                kind: "terminator opcode",
                tag: encoded,
                offset,
            });
        };
        if opcode.class() != AwbcOpcodeClass::Terminator {
            return Err(AwbcCodecError::UnknownTag {
                kind: "terminator opcode",
                tag: encoded,
                offset,
            });
        }
        Ok(match opcode {
            AwbcOpcode::Jump => Self::Jump {
                target: AwbcBlockId::read_wire(reader)?,
            },
            AwbcOpcode::Branch => Self::Branch {
                condition: AwbcRegisterId::read_wire(reader)?,
                then_block: AwbcBlockId::read_wire(reader)?,
                else_block: AwbcBlockId::read_wire(reader)?,
            },
            AwbcOpcode::Match => Self::Match {
                scrutinee: AwbcRegisterId::read_wire(reader)?,
                arms: AwbcTableRange::read_wire(reader)?,
                default: AwbcBlockId::read_wire(reader)?,
            },
            AwbcOpcode::CallFunction => Self::CallFunction {
                function: AwbcFunctionId::read_wire(reader)?,
                args: Vec::<AwbcRegisterId>::read_wire(reader)?,
                dst: Option::<AwbcRegisterId>::read_wire(reader)?,
                resume: AwbcResumePointId::read_wire(reader)?,
            },
            AwbcOpcode::GotoStatic => Self::GotoStatic {
                function: AwbcFunctionId::read_wire(reader)?,
                args: Vec::<AwbcRegisterId>::read_wire(reader)?,
            },
            AwbcOpcode::GotoDynamic => Self::GotoDynamic {
                target: AwbcRegisterId::read_wire(reader)?,
                args: Vec::<AwbcRegisterId>::read_wire(reader)?,
            },
            AwbcOpcode::Dialogue => Self::Dialogue {
                content: AwbcContentUnitId::read_wire(reader)?,
                values: Vec::<AwbcDialogueValueBinding>::read_wire(reader)?,
                effects: Vec::<AwbcDialogueContentEffectBinding>::read_wire(reader)?,
                line_task_captures: Vec::<AwbcRegisterId>::read_wire(reader)?,
                result: AwbcDialogueResultTarget::read_wire(reader)?,
                resume: AwbcResumePointId::read_wire(reader)?,
            },
            AwbcOpcode::Choice => Self::Choice {
                choice: AwbcChoiceId::read_wire(reader)?,
                dst: AwbcRegisterId::read_wire(reader)?,
                resume: AwbcResumePointId::read_wire(reader)?,
            },
            AwbcOpcode::Await => Self::Await {
                handle: AwbcRegisterId::read_wire(reader)?,
                binding: Option::<AwbcPatternId>::read_wire(reader)?,
                observer: Option::<AwbcAwaitObserverResume>::read_wire(reader)?,
                resume: AwbcResumePointId::read_wire(reader)?,
            },
            AwbcOpcode::AwaitMany => Self::AwaitMany {
                plan: AwbcTaskPlanId::read_wire(reader)?,
                source: AwbcRegisterId::read_wire(reader)?,
                binding: Option::<AwbcPatternId>::read_wire(reader)?,
                resume: AwbcResumePointId::read_wire(reader)?,
            },
            AwbcOpcode::HostCall => Self::HostCall {
                call: AwbcHostCallId::read_wire(reader)?,
                args: Vec::<AwbcRegisterId>::read_wire(reader)?,
                dst: Option::<AwbcRegisterId>::read_wire(reader)?,
                resume: AwbcResumePointId::read_wire(reader)?,
            },
            AwbcOpcode::ProjectCall => Self::ProjectCall {
                call: AwbcProjectCall::read_wire(reader)?,
            },
            AwbcOpcode::Return => Self::Return {
                value: Option::<AwbcRegisterId>::read_wire(reader)?,
            },
            AwbcOpcode::Trap => Self::Trap {
                code: AwbcTrapCode::read_wire(reader)?,
                message: Option::<AwbcStringId>::read_wire(reader)?,
            },
            AwbcOpcode::BudgetYield => Self::BudgetYield {
                resume: AwbcResumePointId::read_wire(reader)?,
            },
            AwbcOpcode::Unreachable => Self::Unreachable,
            AwbcOpcode::Nop
            | AwbcOpcode::LoadConst
            | AwbcOpcode::Move
            | AwbcOpcode::CopyValue
            | AwbcOpcode::Clear
            | AwbcOpcode::EnterScope
            | AwbcOpcode::ExitScope
            | AwbcOpcode::BindPattern
            | AwbcOpcode::TestPattern
            | AwbcOpcode::MakeTuple
            | AwbcOpcode::MakeSequence
            | AwbcOpcode::RepeatSequence
            | AwbcOpcode::SequenceLen
            | AwbcOpcode::SequenceGet
            | AwbcOpcode::SequenceSlice
            | AwbcOpcode::SequencePush
            | AwbcOpcode::MakeRecord
            | AwbcOpcode::MakeVariant
            | AwbcOpcode::ProjectTuple
            | AwbcOpcode::ProjectRecord
            | AwbcOpcode::ProjectField
            | AwbcOpcode::Unary
            | AwbcOpcode::Binary
            | AwbcOpcode::CallPureHelper
            | AwbcOpcode::CallIntrinsic
            | AwbcOpcode::EnsureContent
            | AwbcOpcode::MakeDialogueContent
            | AwbcOpcode::EmitEffect
            | AwbcOpcode::StartTask
            | AwbcOpcode::SpawnFiber
            | AwbcOpcode::StreamYield
            | AwbcOpcode::StreamClose
            | AwbcOpcode::ExecuteLineOperation
            | AwbcOpcode::CommitDialogueResult
            | AwbcOpcode::Drop
            | AwbcOpcode::AssignRecordField
            | AwbcOpcode::CallTraitMethod
            | AwbcOpcode::RegisterCleanup
            | AwbcOpcode::CancelCleanup
            | AwbcOpcode::MakeFunction
            | AwbcOpcode::ApplyFunction
            | AwbcOpcode::MakeAgent
            | AwbcOpcode::MakeReductionUnchanged => {
                unreachable!("instruction opcode rejected above")
            }
        })
    }
}

impl Wire for AwbcAwaitObserverResume {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        self.destination.write_wire(writer)?;
        self.resume.write_wire(writer)
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        Ok(Self {
            destination: AwbcRegisterId::read_wire(reader)?,
            resume: AwbcResumePointId::read_wire(reader)?,
        })
    }
}

impl Wire for AwbcBindMode {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        writer.write_u8(self.encoded());
        Ok(())
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        let offset = reader.offset();
        let tag = reader.read_u8()?;
        Self::from_encoded(tag).ok_or(AwbcCodecError::UnknownTag {
            kind: "bind mode",
            tag,
            offset,
        })
    }
}

impl Wire for AwbcUnaryOp {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        writer.write_u8(self.encoded());
        Ok(())
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        let offset = reader.offset();
        let tag = reader.read_u8()?;
        Self::from_encoded(tag).ok_or(AwbcCodecError::UnknownTag {
            kind: "unary operator",
            tag,
            offset,
        })
    }
}

impl Wire for AwbcBinaryOp {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        writer.write_u8(self.encoded());
        Ok(())
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        let offset = reader.offset();
        let tag = reader.read_u8()?;
        Self::from_encoded(tag).ok_or(AwbcCodecError::UnknownTag {
            kind: "binary operator",
            tag,
            offset,
        })
    }
}

impl Wire for RuntimeAgentConstructor {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        writer.write_u8(self.semantic_tag());
        Ok(())
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        let offset = reader.offset();
        let tag = reader.read_u8()?;
        Self::from_semantic_tag(tag).ok_or(AwbcCodecError::UnknownTag {
            kind: "Agent constructor",
            tag,
            offset,
        })
    }
}

impl Wire for AwbcSafePointKind {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        writer.write_u8(self.encoded());
        Ok(())
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        let offset = reader.offset();
        let tag = reader.read_u8()?;
        Self::from_encoded(tag).ok_or(AwbcCodecError::UnknownTag {
            kind: "safe point kind",
            tag,
            offset,
        })
    }
}

impl Wire for AwbcResumePoint {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        self.function.write_wire(writer)?;
        self.block.write_wire(writer)?;
        self.frame_layout.write_wire(writer)?;
        self.kind.write_wire(writer)
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        Ok(Self {
            function: AwbcFunctionId::read_wire(reader)?,
            block: AwbcBlockId::read_wire(reader)?,
            frame_layout: AwbcFrameLayoutId::read_wire(reader)?,
            kind: AwbcSafePointKind::read_wire(reader)?,
        })
    }
}

impl Wire for AwbcTrapCode {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        writer.write_u8(self.encoded());
        Ok(())
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        let offset = reader.offset();
        let tag = reader.read_u8()?;
        Self::from_encoded(tag).ok_or(AwbcCodecError::UnknownTag {
            kind: "trap code",
            tag,
            offset,
        })
    }
}

impl Wire for AwbcPattern {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        match self {
            Self::Bind {
                target,
                mutable,
                expected,
            } => {
                writer.write_u8(0);
                target.write_wire(writer)?;
                mutable.write_wire(writer)?;
                expected.write_wire(writer)?;
            }
            Self::Discard => writer.write_u8(1),
            Self::Literal(value) => {
                writer.write_u8(2);
                value.write_wire(writer)?;
            }
            Self::Entity(value) => {
                writer.write_u8(3);
                value.write_wire(writer)?;
            }
            Self::Tuple(items) => {
                writer.write_u8(4);
                items.write_wire(writer)?;
            }
            Self::Record { ty, fields, rest } => {
                writer.write_u8(5);
                ty.write_wire(writer)?;
                fields.write_wire(writer)?;
                rest.write_wire(writer)?;
            }
            Self::Sequence { items, rest } => {
                writer.write_u8(6);
                items.write_wire(writer)?;
                rest.write_wire(writer)?;
            }
            Self::Variant {
                ty,
                case,
                case_name,
                payload,
            } => {
                writer.write_u8(7);
                ty.write_wire(writer)?;
                case.write_wire(writer)?;
                case_name.write_wire(writer)?;
                payload.write_wire(writer)?;
            }
            Self::Whole { target, inner } => {
                writer.write_u8(8);
                target.write_wire(writer)?;
                inner.write_wire(writer)?;
            }
        }
        Ok(())
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        let offset = reader.offset();
        Ok(match reader.read_u8()? {
            0 => Self::Bind {
                target: AwbcRegisterId::read_wire(reader)?,
                mutable: bool::read_wire(reader)?,
                expected: Option::<AwbcTypeId>::read_wire(reader)?,
            },
            1 => Self::Discard,
            2 => Self::Literal(AwbcConstantId::read_wire(reader)?),
            3 => Self::Entity(crate::value::RuntimeEntityReference::read_wire(reader)?),
            4 => Self::Tuple(Vec::<AwbcPatternId>::read_wire(reader)?),
            5 => Self::Record {
                ty: Option::<AwbcTypeId>::read_wire(reader)?,
                fields: Vec::<AwbcRecordPatternField>::read_wire(reader)?,
                rest: AwbcPatternRest::read_wire(reader)?,
            },
            6 => Self::Sequence {
                items: Vec::<AwbcPatternId>::read_wire(reader)?,
                rest: AwbcPatternRest::read_wire(reader)?,
            },
            7 => Self::Variant {
                ty: AwbcTypeId::read_wire(reader)?,
                case: u32::read_wire(reader)?,
                case_name: AwbcStringId::read_wire(reader)?,
                payload: Option::<AwbcPatternId>::read_wire(reader)?,
            },
            8 => Self::Whole {
                target: AwbcRegisterId::read_wire(reader)?,
                inner: AwbcPatternId::read_wire(reader)?,
            },
            tag => {
                return Err(AwbcCodecError::UnknownTag {
                    kind: "pattern",
                    tag,
                    offset,
                });
            }
        })
    }
}

impl Wire for AwbcPatternRest {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        match self {
            Self::Exact => writer.write_u8(0),
            Self::Ignore => writer.write_u8(1),
            Self::Bind(register) => {
                writer.write_u8(2);
                register.write_wire(writer)?;
            }
        }
        Ok(())
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        let offset = reader.offset();
        match reader.read_u8()? {
            0 => Ok(Self::Exact),
            1 => Ok(Self::Ignore),
            2 => Ok(Self::Bind(AwbcRegisterId::read_wire(reader)?)),
            tag => Err(AwbcCodecError::UnknownTag {
                kind: "pattern rest",
                tag,
                offset,
            }),
        }
    }
}

impl Wire for AwbcRecordPatternField {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        self.field.write_wire(writer)?;
        self.pattern.write_wire(writer)
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        Ok(Self {
            field: u32::read_wire(reader)?,
            pattern: AwbcPatternId::read_wire(reader)?,
        })
    }
}

impl Wire for AwbcMatchArm {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        self.pattern.write_wire(writer)?;
        self.guard.write_wire(writer)?;
        self.target.write_wire(writer)
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        Ok(Self {
            pattern: AwbcPatternId::read_wire(reader)?,
            guard: Option::<AwbcFunctionId>::read_wire(reader)?,
            target: AwbcBlockId::read_wire(reader)?,
        })
    }
}

#[cfg(test)]
mod opcode_class_tests {
    use super::*;
    use crate::awbc::codec::AwbcDecodeBudget;

    #[test]
    fn instruction_decoder_rejects_a_known_terminator_opcode() {
        let bytes = [AwbcOpcode::Jump.encoded()];
        let mut reader = Reader::new(&bytes, &AwbcDecodeBudget::default());
        assert_eq!(
            AwbcInstruction::read_wire(&mut reader)
                .expect_err("terminator cannot enter instruction table"),
            AwbcCodecError::UnknownTag {
                kind: "instruction opcode",
                tag: AwbcOpcode::Jump.encoded(),
                offset: 0,
            }
        );
    }

    #[test]
    fn terminator_decoder_rejects_a_known_instruction_opcode() {
        let bytes = [AwbcOpcode::Nop.encoded()];
        let mut reader = Reader::new(&bytes, &AwbcDecodeBudget::default());
        assert_eq!(
            AwbcTerminator::read_wire(&mut reader)
                .expect_err("instruction cannot enter terminator table"),
            AwbcCodecError::UnknownTag {
                kind: "terminator opcode",
                tag: AwbcOpcode::Nop.encoded(),
                offset: 0,
            }
        );
    }

    #[test]
    fn both_opcode_decoders_reject_an_unassigned_byte() {
        let bytes = [0xff];
        let mut instruction = Reader::new(&bytes, &AwbcDecodeBudget::default());
        assert!(matches!(
            AwbcInstruction::read_wire(&mut instruction),
            Err(AwbcCodecError::UnknownTag {
                kind: "instruction opcode",
                tag: 0xff,
                offset: 0,
            })
        ));
        let mut terminator = Reader::new(&bytes, &AwbcDecodeBudget::default());
        assert!(matches!(
            AwbcTerminator::read_wire(&mut terminator),
            Err(AwbcCodecError::UnknownTag {
                kind: "terminator opcode",
                tag: 0xff,
                offset: 0,
            })
        ));
    }
}

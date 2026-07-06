#![allow(
    clippy::too_many_lines,
    reason = "AWBC wire tables encode large tagged instruction families in one canonical order"
)]

use super::AwbcCodecError;
use super::wire::{Reader, Wire, Writer, wire_enum};
use crate::awbc::schema::{
    AwbcBinaryOp, AwbcBindMode, AwbcBlock, AwbcBlockId, AwbcChoiceId, AwbcConstantId,
    AwbcContentUnitId, AwbcEffectPlanId, AwbcFrameLayoutId, AwbcFunction, AwbcFunctionFlags,
    AwbcFunctionId, AwbcFunctionKind, AwbcHostCallId, AwbcInstruction, AwbcIntrinsicId,
    AwbcLineTaskGroupId, AwbcMatchArm, AwbcOpcode, AwbcPattern, AwbcPatternId, AwbcPureHelperId,
    AwbcRecordPatternField, AwbcRegisterId, AwbcResumePoint, AwbcResumePointId, AwbcSafePointKind,
    AwbcScopeId, AwbcSignatureId, AwbcSourceMapId, AwbcSourcePlanId, AwbcStreamPlanId,
    AwbcStringId, AwbcTableRange, AwbcTaskPlanId, AwbcTerminator, AwbcTraitMethodId, AwbcTrapCode,
    AwbcTypeId, AwbcUnaryOp,
};

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

wire_enum!(AwbcFunctionKind, "function kind", {
    0 => AwbcFunctionKind::Flow,
    1 => AwbcFunctionKind::PureHelper,
    2 => AwbcFunctionKind::TraitMethod,
    3 => AwbcFunctionKind::StreamTransform,
    4 => AwbcFunctionKind::SourceOpen,
    5 => AwbcFunctionKind::SourceHandler,
    6 => AwbcFunctionKind::LineTask,
    7 => AwbcFunctionKind::Synthetic,
});

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

impl Wire for AwbcInstruction {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        writer.write_u8(self.opcode().encoded());
        match self {
            Self::Nop => {}
            Self::LoadConst { dst, constant } => {
                dst.write_wire(writer)?;
                constant.write_wire(writer)?;
            }
            Self::Move { dst, src } => {
                dst.write_wire(writer)?;
                src.write_wire(writer)?;
            }
            Self::Clear { register } | Self::Drop { register } => register.write_wire(writer)?,
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
            Self::SourceClose { source } => source.write_wire(writer)?,
            Self::SourceYield { source, value } => {
                source.write_wire(writer)?;
                value.write_wire(writer)?;
            }
            Self::AssignField {
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
        if opcode.is_terminator() {
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
                field: AwbcStringId::read_wire(reader)?,
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
            AwbcOpcode::SourceClose => Self::SourceClose {
                source: AwbcSourcePlanId::read_wire(reader)?,
            },
            AwbcOpcode::Drop => Self::Drop {
                register: AwbcRegisterId::read_wire(reader)?,
            },
            AwbcOpcode::SourceYield => Self::SourceYield {
                source: AwbcSourcePlanId::read_wire(reader)?,
                value: AwbcRegisterId::read_wire(reader)?,
            },
            AwbcOpcode::AssignField => Self::AssignField {
                target: AwbcRegisterId::read_wire(reader)?,
                field: AwbcStringId::read_wire(reader)?,
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
            | AwbcOpcode::Trap
            | AwbcOpcode::BudgetYield
            | AwbcOpcode::Unreachable => unreachable!("terminator opcode rejected above"),
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
                line_task_group,
                resume,
            } => {
                content.write_wire(writer)?;
                line_task_group.write_wire(writer)?;
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
                task,
                binding,
                resume,
            } => {
                task.write_wire(writer)?;
                binding.write_wire(writer)?;
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
        if !opcode.is_terminator() {
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
                line_task_group: AwbcLineTaskGroupId::read_wire(reader)?,
                resume: AwbcResumePointId::read_wire(reader)?,
            },
            AwbcOpcode::Choice => Self::Choice {
                choice: AwbcChoiceId::read_wire(reader)?,
                dst: AwbcRegisterId::read_wire(reader)?,
                resume: AwbcResumePointId::read_wire(reader)?,
            },
            AwbcOpcode::Await => Self::Await {
                task: AwbcRegisterId::read_wire(reader)?,
                binding: Option::<AwbcPatternId>::read_wire(reader)?,
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
            | AwbcOpcode::EmitEffect
            | AwbcOpcode::StartTask
            | AwbcOpcode::SpawnFiber
            | AwbcOpcode::StreamYield
            | AwbcOpcode::StreamClose
            | AwbcOpcode::SourceClose
            | AwbcOpcode::Drop
            | AwbcOpcode::SourceYield
            | AwbcOpcode::AssignField
            | AwbcOpcode::CallTraitMethod
            | AwbcOpcode::RegisterCleanup
            | AwbcOpcode::CancelCleanup => unreachable!("instruction opcode rejected above"),
        })
    }
}

wire_enum!(AwbcBindMode, "bind mode", {
    0 => AwbcBindMode::Declare,
    1 => AwbcBindMode::Assign,
});

wire_enum!(AwbcUnaryOp, "unary operator", {
    0 => AwbcUnaryOp::Not,
    1 => AwbcUnaryOp::Neg,
});

wire_enum!(AwbcBinaryOp, "binary operator", {
    0 => AwbcBinaryOp::Eq,
    1 => AwbcBinaryOp::Ne,
    2 => AwbcBinaryOp::Lt,
    3 => AwbcBinaryOp::Le,
    4 => AwbcBinaryOp::Gt,
    5 => AwbcBinaryOp::Ge,
    6 => AwbcBinaryOp::Add,
    7 => AwbcBinaryOp::Sub,
    8 => AwbcBinaryOp::Mul,
    9 => AwbcBinaryOp::Div,
    10 => AwbcBinaryOp::And,
    11 => AwbcBinaryOp::Or,
});

wire_enum!(AwbcSafePointKind, "safe point kind", {
    0 => AwbcSafePointKind::FlowEntry,
    1 => AwbcSafePointKind::CallableBoundary,
    2 => AwbcSafePointKind::Dialogue,
    3 => AwbcSafePointKind::Choice,
    4 => AwbcSafePointKind::Await,
    5 => AwbcSafePointKind::AwaitMany,
    6 => AwbcSafePointKind::HostCall,
    7 => AwbcSafePointKind::LoopBackedge,
    8 => AwbcSafePointKind::BudgetYield,
    9 => AwbcSafePointKind::Return,
    10 => AwbcSafePointKind::Trap,
    11 => AwbcSafePointKind::None,
});

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

wire_enum!(AwbcTrapCode, "trap code", {
    0 => AwbcTrapCode::TypeMismatch,
    1 => AwbcTrapCode::UninitializedRegister,
    2 => AwbcTrapCode::InvalidIndex,
    3 => AwbcTrapCode::DivisionByZero,
    4 => AwbcTrapCode::PatternMismatch,
    5 => AwbcTrapCode::MissingDynamicTarget,
    6 => AwbcTrapCode::HostAbiMismatch,
    7 => AwbcTrapCode::CapabilityDenied,
    8 => AwbcTrapCode::ExplicitPanic,
    9 => AwbcTrapCode::InternalInvariant,
});

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
            3 => Self::Entity(AwbcStringId::read_wire(reader)?),
            4 => Self::Tuple(Vec::<AwbcPatternId>::read_wire(reader)?),
            5 => Self::Record {
                ty: Option::<AwbcTypeId>::read_wire(reader)?,
                fields: Vec::<AwbcRecordPatternField>::read_wire(reader)?,
                rest: bool::read_wire(reader)?,
            },
            6 => Self::Sequence {
                items: Vec::<AwbcPatternId>::read_wire(reader)?,
                rest: Option::<AwbcRegisterId>::read_wire(reader)?,
            },
            7 => Self::Variant {
                ty: Option::<AwbcTypeId>::read_wire(reader)?,
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

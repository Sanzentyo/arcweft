//! Inherent typed body metadata; recursively owned children stay on the
//! expression-tree visitor, and referenced definitions resolve in the plan context.

use super::{
    RuntimeAgentExpr, RuntimeCallTarget, RuntimeEntityReferenceField, RuntimeExpr, RuntimeExprKind,
    RuntimeFieldProjection, RuntimeProgressField,
};
use crate::plan::body_semantic::{RuntimeBodySemanticContext, RuntimeBodySemanticError};
use crate::task::semantic::TaskSemanticEncoder;

impl RuntimeExpr {
    #[expect(
        clippy::too_many_lines,
        reason = "one exhaustive expression metadata algebra; owned children remain on the shared walker"
    )]
    pub(crate) fn encode_body_metadata(
        &self,
        context: &RuntimeBodySemanticContext<'_>,
        encoder: &mut TaskSemanticEncoder<'_>,
    ) -> Result<(), RuntimeBodySemanticError> {
        encoder.status()?;
        context.write_type(encoder, self.ty())?;
        encoder.tag(self.kind().semantic_tag());
        encoder.count(self.guard_copy_locals().len());
        for local in self.guard_copy_locals() {
            encoder.enter_element();
            context.write_local(encoder, *local)?;
        }
        match self.kind() {
            RuntimeExprKind::Value(value) => value.encode_static_literal(encoder)?,
            RuntimeExprKind::EntityRef(value) => value.encode_body_identity(encoder),
            RuntimeExprKind::FormatContent {
                template,
                attempt,
                operands,
                project_method,
                project_option,
            } => {
                context.write_content_template(encoder, *template)?;
                encoder.tag(u8::from(attempt.is_some()));
                if let Some(attempt) = attempt {
                    context.write_format_attempt(encoder, *attempt)?;
                }
                encoder.count(operands.len());
                for operand in operands {
                    encoder.enter_element();
                    encoder.digest(operand.definition().as_bytes());
                    encoder.count(operand.parameter().index());
                }
                encoder.tag(u8::from(project_method.is_some()));
                if let Some(method) = project_method {
                    context.write_method(encoder, *method)?;
                }
                encoder.tag(u8::from(*project_option));
            }
            RuntimeExprKind::Agent(agent) => {
                encoder.tag(agent.constructor().semantic_tag());
                if let RuntimeAgentExpr::ChoiceAction { choice } = agent {
                    encoder.string(choice.as_str());
                }
            }
            RuntimeExprKind::CharacterDialogue {
                operation, fields, ..
            } => {
                encoder.tag(match operation { arcweft_interaction_model::dialogue::CharacterDialogueOperation::Factory => 0, arcweft_interaction_model::dialogue::CharacterDialogueOperation::Reconfigure => 1 });
                encoder.count(fields.len());
                for field in fields {
                    encoder.enter_element();
                    encoder.tag(field.coordinate.semantic_tag());
                    if let arcweft_interaction_model::dialogue::CharacterDialogueFieldCoordinate::Custom(identity) = &field.coordinate { encoder.string(identity.as_str()); }
                    encoder.tag(match field.operation { arcweft_interaction_model::dialogue::CharacterDialoguePatchOperation::Set(_) => 0, arcweft_interaction_model::dialogue::CharacterDialoguePatchOperation::Clear => 1 });
                }
            }
            RuntimeExprKind::MakeCallable { state, captures } => {
                context.write_callable_state(encoder, *state)?;
                encoder.count(captures.len());
            }
            RuntimeExprKind::DialogueContent {
                template,
                values,
                effects,
            } => {
                context.write_content_template(encoder, *template)?;
                encoder.count(values.len());
                encoder.count(effects.len());
                for effect in effects {
                    encoder.enter_element();
                    encoder.ordinal(effect.site.get().get() - 1);
                    context.write_callable_state(encoder, effect.state)?;
                    encoder.count(effect.captures.len());
                }
            }
            RuntimeExprKind::Call { callee, args } => {
                match callee {
                    RuntimeCallTarget::Intrinsic(intrinsic) => {
                        encoder.tag(0);
                        encoder.tag(intrinsic.semantic_tag());
                    }
                    RuntimeCallTarget::Callable(callable) => {
                        encoder.tag(1);
                        encoder.string(callable.as_str());
                    }
                }
                context.write_call_arguments(encoder, args)?;
            }
            RuntimeExprKind::Field { field, .. } => field.encode_body_metadata(encoder),
            RuntimeExprKind::Local(read) => {
                context.write_local(encoder, read.local())?;
                encoder.tag(read.mode().semantic_tag());
                RuntimeBodySemanticContext::write_fields(encoder, read.fields());
            }
            RuntimeExprKind::SequencePopFront { place }
            | RuntimeExprKind::SequencePopBack { place }
            | RuntimeExprKind::SequencePush { place, .. } => context.write_place(encoder, place)?,
            RuntimeExprKind::Let { binding, .. } => context.write_local(encoder, *binding)?,
            RuntimeExprKind::Tuple(items) | RuntimeExprKind::BracketSeq(items) => {
                encoder.count(items.len());
            }
            RuntimeExprKind::RepeatSeq { len, .. } => encoder.count(*len),
            RuntimeExprKind::Range {
                start,
                end,
                inclusive,
            } => {
                encoder.tag(u8::from(start.is_some()));
                encoder.tag(u8::from(end.is_some()));
                encoder.tag(u8::from(*inclusive));
            }
            RuntimeExprKind::NominalRecord(record) => {
                encoder.count(record.initializers().len());
                for field in record.initializers() {
                    encoder.enter_element();
                    encoder.ordinal(field.field().zero_based());
                }
            }
            RuntimeExprKind::Variant { ordinal, payload } => {
                encoder.ordinal(*ordinal);
                encoder.tag(u8::from(payload.is_some()));
            }
            RuntimeExprKind::ProjectTuple { ordinal, .. }
            | RuntimeExprKind::ProjectRecord { ordinal, .. } => encoder.count(*ordinal),
            RuntimeExprKind::Assign { place, .. } => context.write_assignment(encoder, place)?,
            RuntimeExprKind::TraitCall {
                callable,
                receiver_mode,
                args,
                ..
            } => {
                context.write_method(encoder, *callable)?;
                encoder.tag(match receiver_mode {
                    crate::plan::RuntimeReceiverMode::Owned => 0,
                    crate::plan::RuntimeReceiverMode::SharedRef => 1,
                    crate::plan::RuntimeReceiverMode::MutRef => 2,
                });
                context.write_call_arguments(encoder, args)?;
            }
            RuntimeExprKind::PureCall { helper, args } => {
                context.write_helper(encoder, *helper)?;
                context.write_call_arguments(encoder, args)?;
            }
            RuntimeExprKind::ApplyGroup { args, .. } => {
                context.write_call_arguments(encoder, args)?;
            }
            RuntimeExprKind::StandardMap { family, order, .. } => {
                encoder.tag(family.semantic_tag());
                encoder.tag(order.semantic_tag());
            }
            RuntimeExprKind::Unary { op, .. } => encoder.tag(op.semantic_tag()),
            RuntimeExprKind::Binary { op, .. } => encoder.tag(op.semantic_tag()),
            RuntimeExprKind::IfLet { guard, .. } => encoder.tag(u8::from(guard.is_some())),
            RuntimeExprKind::Match { arms, .. } => {
                encoder.count(arms.len());
                for arm in arms {
                    encoder.enter_element();
                    encoder.tag(u8::from(arm.guard().is_some()));
                }
            }
            RuntimeExprKind::Scope { .. }
            | RuntimeExprKind::Sum { .. }
            | RuntimeExprKind::If { .. }
            | RuntimeExprKind::ReductionUnchanged { .. } => {}
            RuntimeExprKind::SpecializeCallable { specialization, .. } => {
                context.write_specialization(encoder, *specialization)?;
            }
        }
        encoder.status().map_err(Into::into)
    }
}

impl RuntimeFieldProjection {
    fn encode_body_metadata(&self, encoder: &mut TaskSemanticEncoder<'_>) {
        match self {
            Self::Nominal(field) => {
                encoder.tag(0);
                encoder.ordinal(field.zero_based());
            }
            Self::OpaqueRecord { owner, field } => {
                encoder.tag(1);
                encoder.string(owner.producer().as_str());
                encoder.digest(owner.semantic_identity().as_bytes());
                encoder.tag(owner.admission().encoded());
                encoder.tag(owner.value_class().semantic_tag());
                encoder.tag(owner.persistence().semantic_tag());
                encoder.ordinal(field.zero_based());
            }
            Self::Agent(field) => {
                encoder.tag(2);
                encoder.ordinal(u32::from(field.semantic_tag()));
            }
            Self::EntityReference(field) => {
                encoder.tag(3);
                encoder.tag(match field {
                    RuntimeEntityReferenceField::Id => 0,
                    RuntimeEntityReferenceField::Family => 1,
                    RuntimeEntityReferenceField::Name => 2,
                });
            }
            Self::Progress(field) => {
                encoder.tag(4);
                encoder.tag(match field {
                    RuntimeProgressField::Ratio => 0,
                    RuntimeProgressField::Label => 1,
                });
            }
        }
    }
}

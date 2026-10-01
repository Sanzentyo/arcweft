//! Deep Copy admission for literals embedded in an unevaluated expression.

use super::{RuntimeExpr, RuntimeExprKind};
use crate::pattern::{RuntimePattern, RuntimePatternKind};

fn pattern_literals_permit_copy(pattern: &RuntimePattern) -> bool {
    match pattern.kind() {
        RuntimePatternKind::Literal(value) => value.ownership().permits_copy(),
        RuntimePatternKind::Tuple(items) | RuntimePatternKind::Sequence { items, .. } => {
            items.iter().all(pattern_literals_permit_copy)
        }
        RuntimePatternKind::Record { fields, .. } => fields
            .iter()
            .all(|field| pattern_literals_permit_copy(field.pattern())),
        RuntimePatternKind::Variant { payload, .. } => payload
            .as_ref()
            .is_none_or(|payload| pattern_literals_permit_copy(payload)),
        RuntimePatternKind::Whole { pattern, .. } => pattern_literals_permit_copy(pattern),
        RuntimePatternKind::Bind { .. }
        | RuntimePatternKind::Discard
        | RuntimePatternKind::Entity(_)
        | RuntimePatternKind::Typed { .. } => true,
    }
}

impl RuntimeExpr {
    /// An unevaluated expression may be copied into an inert control image
    /// only when every embedded live value is recursively unrestricted.
    /// Local reads are evaluated later and do not duplicate their bindings.
    pub(crate) fn literals_permit_copy(&self) -> bool {
        match self.kind() {
            RuntimeExprKind::Value(value) => value.ownership().permits_copy(),
            RuntimeExprKind::Agent(agent) => {
                agent.operands().into_iter().all(Self::literals_permit_copy)
            }
            RuntimeExprKind::Local(_)
            | RuntimeExprKind::SequencePopFront { .. }
            | RuntimeExprKind::SequencePopBack { .. }
            | RuntimeExprKind::EntityRef(_) => true,
            RuntimeExprKind::SequencePush { value, .. }
            | RuntimeExprKind::Scope { body: value, .. }
            | RuntimeExprKind::RepeatSeq { value, .. }
            | RuntimeExprKind::Field { target: value, .. }
            | RuntimeExprKind::ProjectTuple { target: value, .. }
            | RuntimeExprKind::ProjectRecord { target: value, .. }
            | RuntimeExprKind::SpecializeCallable { value, .. }
            | RuntimeExprKind::Sum { source: value }
            | RuntimeExprKind::Unary { expr: value, .. }
            | RuntimeExprKind::ReductionUnchanged { state: value } => value.literals_permit_copy(),
            RuntimeExprKind::Let { expr, body, .. }
            | RuntimeExprKind::Assign { expr, body, .. } => {
                expr.literals_permit_copy() && body.literals_permit_copy()
            }
            RuntimeExprKind::DialogueContent {
                values, effects, ..
            } => {
                values.iter().all(Self::literals_permit_copy)
                    && effects
                        .iter()
                        .all(|effect| effect.captures.iter().all(Self::literals_permit_copy))
            }
            RuntimeExprKind::FormatContent { operands, .. } => operands
                .iter()
                .all(|operand| operand.expression().literals_permit_copy()),
            RuntimeExprKind::CharacterDialogue { target, fields, .. } => {
                target.literals_permit_copy()
                    && fields.iter().all(|field| {
                        match &field.operation {
                    arcweft_interaction_model::dialogue::CharacterDialoguePatchOperation::Set(
                        value,
                    ) => value.literals_permit_copy(),
                    arcweft_interaction_model::dialogue::CharacterDialoguePatchOperation::Clear => {
                        true
                    }
                }
                    })
            }
            RuntimeExprKind::Tuple(values)
            | RuntimeExprKind::BracketSeq(values)
            | RuntimeExprKind::MakeCallable {
                captures: values, ..
            } => values.iter().all(Self::literals_permit_copy),
            RuntimeExprKind::Range { start, end, .. } => start
                .iter()
                .chain(end.iter())
                .all(|value| value.literals_permit_copy()),
            RuntimeExprKind::NominalRecord(record) => record
                .initializers()
                .iter()
                .all(|initializer| initializer.value().literals_permit_copy()),
            RuntimeExprKind::Variant { payload, .. } => payload
                .as_ref()
                .is_none_or(|value| value.literals_permit_copy()),
            RuntimeExprKind::Call { args, .. } | RuntimeExprKind::PureCall { args, .. } => {
                args.iter().all(|arg| arg.value().literals_permit_copy())
            }
            RuntimeExprKind::ApplyGroup { callee, args } => {
                callee.literals_permit_copy()
                    && args.iter().all(|arg| arg.value().literals_permit_copy())
            }
            RuntimeExprKind::TraitCall { receiver, args, .. } => {
                receiver.literals_permit_copy()
                    && args.iter().all(|arg| arg.value().literals_permit_copy())
            }
            RuntimeExprKind::StandardMap {
                mapping, source, ..
            } => mapping.literals_permit_copy() && source.literals_permit_copy(),
            RuntimeExprKind::Binary { lhs, rhs, .. } => {
                lhs.literals_permit_copy() && rhs.literals_permit_copy()
            }
            RuntimeExprKind::If {
                condition,
                then_expr,
                else_expr,
            } => {
                condition.literals_permit_copy()
                    && then_expr.literals_permit_copy()
                    && else_expr.literals_permit_copy()
            }
            RuntimeExprKind::IfLet {
                pattern,
                expr,
                guard,
                then_expr,
                else_expr,
                ..
            } => {
                pattern_literals_permit_copy(pattern)
                    && expr.literals_permit_copy()
                    && guard
                        .as_ref()
                        .is_none_or(|guard| guard.literals_permit_copy())
                    && then_expr.literals_permit_copy()
                    && else_expr.literals_permit_copy()
            }
            RuntimeExprKind::Match { scrutinee, arms } => {
                scrutinee.literals_permit_copy()
                    && arms.iter().all(|arm| {
                        pattern_literals_permit_copy(arm.pattern())
                            && arm.guard().is_none_or(Self::literals_permit_copy)
                            && arm.value().literals_permit_copy()
                    })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime_id::RuntimePlanTypeId;
    use crate::task::NeedId;
    use crate::value::RuntimeValue;
    use std::num::NonZeroU32;

    #[test]
    fn nested_affine_literal_does_not_admit_an_expression_copy() {
        let ty = RuntimePlanTypeId::from_accepted_ordinal(NonZeroU32::MIN);
        let expression = RuntimeExpr::from_admitted_parts(
            ty,
            RuntimeExprKind::Tuple(vec![
                RuntimeExpr::from_admitted_parts(ty, RuntimeExprKind::Value(RuntimeValue::i64(7))),
                RuntimeExpr::from_admitted_parts(
                    ty,
                    RuntimeExprKind::Value(RuntimeValue::Need(NeedId("need.one".to_owned()))),
                ),
            ]),
        );
        assert!(!expression.literals_permit_copy());

        let copy =
            RuntimeExpr::from_admitted_parts(ty, RuntimeExprKind::Value(RuntimeValue::i64(7)));
        let match_expression = RuntimeExpr::from_admitted_parts(
            ty,
            RuntimeExprKind::IfLet {
                pattern: RuntimePattern::from_admitted_parts(
                    ty,
                    RuntimePatternKind::Literal(RuntimeValue::Need(NeedId(
                        "need.pattern".to_owned(),
                    ))),
                ),
                expr: Box::new(copy.clone()),
                guard: None,
                then_expr: Box::new(copy.clone()),
                else_expr: Box::new(copy),
            },
        );
        assert!(!match_expression.literals_permit_copy());
    }
}

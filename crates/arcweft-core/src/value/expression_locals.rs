//! Exact free-local projection for admitted runtime expressions.

use super::{
    RuntimeCallArgument, RuntimeExpr, RuntimeExprKind, RuntimeLocalReadMode, RuntimeMutablePlace,
    RuntimeStandardMapOperandOrder,
};
use crate::pattern::RuntimePatternBindingDeclaration;
use crate::plan::RuntimePlan;
use crate::runtime_id::{RuntimeFunctionSiteId, RuntimeLocalDeclarationId};
use thiserror::Error;

/// Failure to resolve the complete plan context required by an admitted
/// expression's free-local projection.
#[derive(Clone, Copy, Debug, Eq, Error, Hash, Ord, PartialEq, PartialOrd)]
pub enum RuntimeExprFreeLocalError {
    #[error("runtime expression references unknown function site {site}")]
    UnknownFunctionSite { site: RuntimeFunctionSiteId },
    #[error("runtime expression references unknown callable state {state}")]
    UnknownCallableState {
        state: crate::runtime_id::RuntimeCallableStateId,
    },
}

#[derive(Clone, Copy)]
struct FreeLocalUse {
    local: RuntimeLocalDeclarationId,
    mode: RuntimeLocalReadMode,
}

impl RuntimeExpr {
    /// Returns the locals required to evaluate this expression in deterministic
    /// first-use order.
    ///
    /// Bindings owned inside the expression are excluded. Constructing an
    /// explicit function value contributes that function site's declared
    /// captures because an evaluator must have those values available at the
    /// construction point.
    pub fn evaluation_free_locals(
        &self,
        plan: &RuntimePlan,
    ) -> Result<Box<[RuntimeLocalDeclarationId]>, RuntimeExprFreeLocalError> {
        Ok(self
            .evaluation_free_local_reads(plan)?
            .into_vec()
            .into_iter()
            .map(|(local, _)| local)
            .collect::<Vec<_>>()
            .into_boxed_slice())
    }

    /// Returns free locals in deterministic first-use order together with the
    /// strictest transfer mode admitted for each capture. Repeated uses merge
    /// to `Move` if any occurrence is affine; only all-Copy reads permit a
    /// copied closure capture.
    pub fn evaluation_free_local_reads(
        &self,
        plan: &RuntimePlan,
    ) -> Result<Box<[(RuntimeLocalDeclarationId, RuntimeLocalReadMode)]>, RuntimeExprFreeLocalError>
    {
        let mut locals = Vec::new();
        self.collect_evaluation_free_locals(plan, &[], &mut locals)?;
        Ok(locals
            .into_iter()
            .map(|use_| (use_.local, use_.mode))
            .collect::<Vec<_>>()
            .into_boxed_slice())
    }

    fn collect_evaluation_free_locals(
        &self,
        plan: &RuntimePlan,
        bound: &[RuntimeLocalDeclarationId],
        locals: &mut Vec<FreeLocalUse>,
    ) -> Result<(), RuntimeExprFreeLocalError> {
        match self.kind() {
            RuntimeExprKind::Value(_) | RuntimeExprKind::EntityRef(_) => {}
            RuntimeExprKind::Agent(agent) => {
                for operand in agent.operands() {
                    operand.collect_evaluation_free_locals(plan, bound, locals)?;
                }
            }
            RuntimeExprKind::Local(read) => {
                push_free_local_with_mode(read.local(), read.mode(), bound, locals)
            }
            RuntimeExprKind::SequencePopFront { place } => match place {
                RuntimeMutablePlace::Local(local) => push_free_local(*local, bound, locals),
                RuntimeMutablePlace::Fields { base, .. } => push_free_local(*base, bound, locals),
            },
            RuntimeExprKind::SequencePush { place, value } => {
                match place {
                    RuntimeMutablePlace::Local(local) => push_free_local(*local, bound, locals),
                    RuntimeMutablePlace::Fields { base, .. } => {
                        push_free_local(*base, bound, locals)
                    }
                }
                value.collect_evaluation_free_locals(plan, bound, locals)?;
            }
            RuntimeExprKind::SequencePopBack { place } => match place {
                RuntimeMutablePlace::Local(local) => push_free_local(*local, bound, locals),
                RuntimeMutablePlace::Fields { base, .. } => push_free_local(*base, bound, locals),
            },
            RuntimeExprKind::Let {
                binding,
                expr,
                body,
            } => {
                expr.collect_evaluation_free_locals(plan, bound, locals)?;
                let mut body_bound = bound.to_vec();
                body_bound.push(*binding);
                body.collect_evaluation_free_locals(plan, &body_bound, locals)?;
            }
            RuntimeExprKind::Scope { body, .. } => {
                body.collect_evaluation_free_locals(plan, bound, locals)?;
            }
            RuntimeExprKind::DialogueContent {
                values, effects, ..
            } => {
                collect_slice_free_locals(plan, values, bound, locals)?;
                for effect in effects {
                    collect_slice_free_locals(plan, &effect.captures, bound, locals)?;
                }
            }
            RuntimeExprKind::FormatContent { operands, .. } => {
                for operand in operands {
                    operand
                        .expression()
                        .collect_evaluation_free_locals(plan, bound, locals)?;
                }
            }
            RuntimeExprKind::CharacterDialogue { target, fields, .. } => {
                target.collect_evaluation_free_locals(plan, bound, locals)?;
                for field in fields {
                    if let arcweft_interaction_model::dialogue::CharacterDialoguePatchOperation::Set(
                        value,
                    ) = &field.operation
                    {
                        value.collect_evaluation_free_locals(plan, bound, locals)?;
                    }
                }
            }
            RuntimeExprKind::Tuple(items) | RuntimeExprKind::BracketSeq(items) => {
                collect_slice_free_locals(plan, items, bound, locals)?;
            }
            RuntimeExprKind::RepeatSeq { value, .. }
            | RuntimeExprKind::ProjectTuple { target: value, .. }
            | RuntimeExprKind::ProjectRecord { target: value, .. }
            | RuntimeExprKind::Sum { source: value }
            | RuntimeExprKind::Unary { expr: value, .. }
            | RuntimeExprKind::ReductionUnchanged { state: value } => {
                value.collect_evaluation_free_locals(plan, bound, locals)?;
            }
            RuntimeExprKind::Field { target, .. } => match target {
                super::RuntimeFieldTarget::Value(value) => {
                    value.collect_evaluation_free_locals(plan, bound, locals)?
                }
                super::RuntimeFieldTarget::Inspect { place, .. } => {
                    push_free_local(place.local(), bound, locals)
                }
            },
            RuntimeExprKind::Range { start, end, .. } => {
                for value in start.iter().chain(end.iter()) {
                    value.collect_evaluation_free_locals(plan, bound, locals)?;
                }
            }
            RuntimeExprKind::NominalRecord(record) => {
                for initializer in record.initializers() {
                    initializer
                        .value()
                        .collect_evaluation_free_locals(plan, bound, locals)?;
                }
            }
            RuntimeExprKind::Variant { payload, .. } => {
                if let Some(payload) = payload {
                    payload.collect_evaluation_free_locals(plan, bound, locals)?;
                }
            }
            RuntimeExprKind::Assign { place, expr, body } => {
                push_free_local(place.local(), bound, locals);
                expr.collect_evaluation_free_locals(plan, bound, locals)?;
                body.collect_evaluation_free_locals(plan, bound, locals)?;
            }
            RuntimeExprKind::Call { args, .. } | RuntimeExprKind::PureCall { args, .. } => {
                collect_argument_free_locals(plan, args, bound, locals)?;
            }
            RuntimeExprKind::MakeCallable { state, captures } => {
                plan.callable_states()
                    .get(*state)
                    .ok_or(RuntimeExprFreeLocalError::UnknownCallableState { state: *state })?;
                collect_slice_free_locals(plan, captures, bound, locals)?;
            }
            RuntimeExprKind::SpecializeCallable { value, .. } => {
                value.collect_evaluation_free_locals(plan, bound, locals)?;
            }
            RuntimeExprKind::ApplyGroup { callee, args } => {
                callee.collect_evaluation_free_locals(plan, bound, locals)?;
                collect_argument_free_locals(plan, args, bound, locals)?;
            }
            RuntimeExprKind::TraitCall { receiver, args, .. } => {
                receiver.collect_evaluation_free_locals(plan, bound, locals)?;
                collect_argument_free_locals(plan, args, bound, locals)?;
            }
            RuntimeExprKind::StandardMap {
                order,
                mapping,
                source,
                ..
            } => match order {
                RuntimeStandardMapOperandOrder::MappingThenReceiver => {
                    mapping.collect_evaluation_free_locals(plan, bound, locals)?;
                    source.collect_evaluation_free_locals(plan, bound, locals)?;
                }
                RuntimeStandardMapOperandOrder::ReceiverThenMapping => {
                    source.collect_evaluation_free_locals(plan, bound, locals)?;
                    mapping.collect_evaluation_free_locals(plan, bound, locals)?;
                }
            },
            RuntimeExprKind::Binary { lhs, rhs, .. } => {
                lhs.collect_evaluation_free_locals(plan, bound, locals)?;
                rhs.collect_evaluation_free_locals(plan, bound, locals)?;
            }
            RuntimeExprKind::If {
                condition,
                then_expr,
                else_expr,
            } => {
                condition.collect_evaluation_free_locals(plan, bound, locals)?;
                then_expr.collect_evaluation_free_locals(plan, bound, locals)?;
                else_expr.collect_evaluation_free_locals(plan, bound, locals)?;
            }
            RuntimeExprKind::IfLet {
                pattern,
                expr,
                guard,
                then_expr,
                else_expr,
            } => {
                expr.collect_evaluation_free_locals(plan, bound, locals)?;
                let mut branch_bound = bound.to_vec();
                branch_bound.extend(
                    pattern
                        .binding_declarations()
                        .map(RuntimePatternBindingDeclaration::local),
                );
                if let Some(guard) = guard {
                    guard.collect_evaluation_free_locals(plan, &branch_bound, locals)?;
                }
                then_expr.collect_evaluation_free_locals(plan, &branch_bound, locals)?;
                else_expr.collect_evaluation_free_locals(plan, bound, locals)?;
            }
            RuntimeExprKind::Match { scrutinee, arms } => {
                scrutinee.collect_evaluation_free_locals(plan, bound, locals)?;
                for arm in arms {
                    let mut arm_bound = bound.to_vec();
                    arm_bound.extend(
                        arm.pattern()
                            .binding_declarations()
                            .map(RuntimePatternBindingDeclaration::local),
                    );
                    if let Some(guard) = arm.guard() {
                        guard.collect_evaluation_free_locals(plan, &arm_bound, locals)?;
                    }
                    arm.value()
                        .collect_evaluation_free_locals(plan, &arm_bound, locals)?;
                }
            }
        }
        Ok(())
    }
}

fn collect_slice_free_locals(
    plan: &RuntimePlan,
    expressions: &[RuntimeExpr],
    bound: &[RuntimeLocalDeclarationId],
    locals: &mut Vec<FreeLocalUse>,
) -> Result<(), RuntimeExprFreeLocalError> {
    for expression in expressions {
        expression.collect_evaluation_free_locals(plan, bound, locals)?;
    }
    Ok(())
}

fn collect_argument_free_locals(
    plan: &RuntimePlan,
    arguments: &[RuntimeCallArgument],
    bound: &[RuntimeLocalDeclarationId],
    locals: &mut Vec<FreeLocalUse>,
) -> Result<(), RuntimeExprFreeLocalError> {
    for argument in arguments {
        argument
            .value()
            .collect_evaluation_free_locals(plan, bound, locals)?;
    }
    Ok(())
}

fn push_free_local(
    local: RuntimeLocalDeclarationId,
    bound: &[RuntimeLocalDeclarationId],
    locals: &mut Vec<FreeLocalUse>,
) {
    push_free_local_with_mode(local, RuntimeLocalReadMode::Move, bound, locals);
}

fn push_free_local_with_mode(
    local: RuntimeLocalDeclarationId,
    mode: RuntimeLocalReadMode,
    bound: &[RuntimeLocalDeclarationId],
    locals: &mut Vec<FreeLocalUse>,
) {
    if bound.contains(&local) {
        return;
    }
    if let Some(existing) = locals.iter_mut().find(|use_| use_.local == local) {
        if mode == RuntimeLocalReadMode::Move {
            existing.mode = RuntimeLocalReadMode::Move;
        }
    } else {
        locals.push(FreeLocalUse { local, mode });
    }
}

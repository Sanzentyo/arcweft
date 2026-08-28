//! Exact free-local projection for admitted runtime expressions.

use super::{RuntimeCallArgument, RuntimeExpr, RuntimeExprKind, RuntimeStandardMapOperandOrder};
use crate::pattern::{RuntimePattern, RuntimePatternKind};
use crate::plan::RuntimePlan;
use crate::runtime_id::{RuntimeFunctionSiteId, RuntimeLocalDeclarationId};
use thiserror::Error;

/// Failure to resolve the complete plan context required by an admitted
/// expression's free-local projection.
#[derive(Clone, Copy, Debug, Eq, Error, Hash, Ord, PartialEq, PartialOrd)]
pub enum RuntimeExprFreeLocalError {
    #[error("runtime expression references unknown function site {site}")]
    UnknownFunctionSite { site: RuntimeFunctionSiteId },
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
        let mut locals = Vec::new();
        self.collect_evaluation_free_locals(plan, &[], &mut locals)?;
        Ok(locals.into_boxed_slice())
    }

    fn collect_evaluation_free_locals(
        &self,
        plan: &RuntimePlan,
        bound: &[RuntimeLocalDeclarationId],
        locals: &mut Vec<RuntimeLocalDeclarationId>,
    ) -> Result<(), RuntimeExprFreeLocalError> {
        match self.kind() {
            RuntimeExprKind::Value(_) | RuntimeExprKind::EntityRef(_) => {}
            RuntimeExprKind::Agent(agent) => {
                for operand in agent.operands() {
                    operand.collect_evaluation_free_locals(plan, bound, locals)?;
                }
            }
            RuntimeExprKind::Local(local) => push_free_local(*local, bound, locals),
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
            RuntimeExprKind::Tuple(items) | RuntimeExprKind::BracketSeq(items) => {
                collect_slice_free_locals(plan, items, bound, locals)?;
            }
            RuntimeExprKind::RepeatSeq { value, .. }
            | RuntimeExprKind::Field { target: value, .. }
            | RuntimeExprKind::ProjectTuple { target: value, .. }
            | RuntimeExprKind::ProjectRecord { target: value, .. }
            | RuntimeExprKind::Sum { source: value }
            | RuntimeExprKind::Unary { expr: value, .. }
            | RuntimeExprKind::ReductionUnchanged { state: value } => {
                value.collect_evaluation_free_locals(plan, bound, locals)?;
            }
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
            RuntimeExprKind::AssignNominalField {
                base, expr, body, ..
            } => {
                push_free_local(*base, bound, locals);
                expr.collect_evaluation_free_locals(plan, bound, locals)?;
                body.collect_evaluation_free_locals(plan, bound, locals)?;
            }
            RuntimeExprKind::Call { args, .. } | RuntimeExprKind::PureCall { args, .. } => {
                collect_argument_free_locals(plan, args, bound, locals)?;
            }
            RuntimeExprKind::Function(site) => {
                let function = plan
                    .function_sites()
                    .get(*site)
                    .ok_or(RuntimeExprFreeLocalError::UnknownFunctionSite { site: *site })?;
                for capture in function.captures() {
                    push_free_local(*capture, bound, locals);
                }
            }
            RuntimeExprKind::Apply { callee, args } => {
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
            RuntimeExprKind::Filter {
                source,
                param,
                body,
            } => {
                source.collect_evaluation_free_locals(plan, bound, locals)?;
                let mut body_bound = bound.to_vec();
                body_bound.push(*param);
                body.collect_evaluation_free_locals(plan, &body_bound, locals)?;
            }
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
                collect_pattern_bindings(pattern, &mut branch_bound);
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
                    collect_pattern_bindings(arm.pattern(), &mut arm_bound);
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
    locals: &mut Vec<RuntimeLocalDeclarationId>,
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
    locals: &mut Vec<RuntimeLocalDeclarationId>,
) -> Result<(), RuntimeExprFreeLocalError> {
    for argument in arguments {
        argument
            .value()
            .collect_evaluation_free_locals(plan, bound, locals)?;
    }
    Ok(())
}

fn collect_pattern_bindings(pattern: &RuntimePattern, bound: &mut Vec<RuntimeLocalDeclarationId>) {
    match pattern.kind() {
        RuntimePatternKind::Bind { binding, .. } | RuntimePatternKind::Typed { binding } => {
            bound.push(binding.local());
        }
        RuntimePatternKind::Discard
        | RuntimePatternKind::Literal(_)
        | RuntimePatternKind::Entity(_) => {}
        RuntimePatternKind::Tuple(items) => {
            for item in items {
                collect_pattern_bindings(item, bound);
            }
        }
        RuntimePatternKind::Record { fields, rest } => {
            for field in fields {
                collect_pattern_bindings(field.pattern(), bound);
            }
            if let Some(binding) = rest.binding() {
                bound.push(binding.local());
            }
        }
        RuntimePatternKind::Sequence { items, rest } => {
            for item in items {
                collect_pattern_bindings(item, bound);
            }
            if let Some(binding) = rest.binding() {
                bound.push(binding.local());
            }
        }
        RuntimePatternKind::Variant { payload, .. } => {
            if let Some(payload) = payload {
                collect_pattern_bindings(payload, bound);
            }
        }
        RuntimePatternKind::Whole { binding, pattern } => {
            bound.push(binding.local());
            collect_pattern_bindings(pattern, bound);
        }
    }
}

fn push_free_local(
    local: RuntimeLocalDeclarationId,
    bound: &[RuntimeLocalDeclarationId],
    locals: &mut Vec<RuntimeLocalDeclarationId>,
) {
    if !bound.contains(&local) && !locals.contains(&local) {
        locals.push(local);
    }
}
